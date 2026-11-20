# Package update process fixtures

The Linux x86-64 candidate for commit 4d0267f failed the detached-terminal fixture with an unreported error. The other four native targets passed. The failure was reproduced using the unchanged package-update module in an isolated Ubuntu 24.04 container: executing a temporary script returned ETXTBSY (errno 26). This did not involve a real terminal, package manager transaction or installed application.

## Evidence and change

Each fixture uses a private directory whose name contains the process ID, nanosecond timestamp and an atomic counter. The Rust file writer closes before the test executes its script. Those properties isolate paths; the concurrent tests still share one process file-descriptor table.

A separate, deterministic Linux experiment demonstrated the relevant inheritance mechanism: open a script with O_WRONLY and O_CLOEXEC, fork a child, close the parent's writer, then execute the script before the child execs. The parent's writer was closed, the child's fdinfo still recorded write flags 02100001, and execution failed with errno 26. Execution succeeded immediately after the child execed. This demonstrates the mechanism; it does not identify the descriptor owner in the original CI failure.

The four fixtures that create and execute scripts now share a mutex covering construction and process execution. The detached-terminal script is constructed in its final form before discovery. The assertion also retains the actual error if a future run fails. No production update logic or dependency changed.

## Validation

- The normal local GUI test command passed all 15 tests.
- An isolated Rust harness containing the same package-update module passed all four fixtures under Ubuntu 24.04's normal parallel test runner. The harness substitutes only the text-formatting functions; it does not claim to run the full GUI in the container.
- The previous source passed the same four fixtures when run serially, and the detached fixture passed on its own.
- The final native candidate must run on the new commit; the failed 4d0267f candidate is not treated as a successful release gate.

## Production boundary

The package manager path discovers installed terminal and helper executables; the GUI does not generate or rewrite either program. Its generated files contain results and are not executable.

The standalone updater writes downloads through a separate curl child and waits for that child to exit before checking SHA-256 and executing the downloaded version probe. The GUI busy guard permits one action at a time. The command-line update path runs synchronously before creating a window or background action threads.

Relevant code: [package update](../../src/package_update.rs), [standalone updater](../../src/updater.rs), [GUI action guard](../../src/main.rs).
