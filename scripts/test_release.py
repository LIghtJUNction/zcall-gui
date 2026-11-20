#!/usr/bin/env python3
"""Focused release transactions; only temporary repositories are modified."""
import contextlib,io,pathlib,shutil,subprocess,tempfile,tomllib,unittest
from unittest.mock import patch
import release,aur

class VersionTransaction(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix="zcall-gui-release-test-")
        self.root=pathlib.Path(self.temp.name);self.previous=release.ROOT;release.ROOT=self.root
        (self.root/"src").mkdir();(self.root/"src/main.rs").write_text("fn main() {}\n")
        (self.root/"Cargo.toml").write_text('[package]\nname = "zcall-gui"\nversion = "0.1.0"\nedition = "2024"\n[dependencies]\nzgpui = { path = "vendor/zgpui", version = "=0.1.1", features = ["native"] }\n')
        vendor=self.root/"vendor/zgpui";(vendor/"src").mkdir(parents=True)
        (vendor/"Cargo.toml").write_text('[package]\nname = "zgpui"\nversion = "0.1.1"\nedition = "2024"\n[features]\ndefault = []\nstd = []\nnative = ["std"]\n')
        (vendor/"src/lib.rs").write_text("pub fn example() {}\n")
        (self.root/"CHANGELOG.md").write_text("# Changelog\n\n## Unreleased\n\n- Native UI changes.\n")
        self.cmd("cargo","generate-lockfile","--offline")
        self.cmd("git","init","-q")
        self.cmd("git","config","user.name","Release test")
        self.cmd("git","config","user.email","test@example.invalid")
        self.cmd("git","config","commit.gpgsign","false")
        self.cmd("git","add",".");self.cmd("git","commit","-qm","Initial fixture")

    def tearDown(self):
        release.ROOT=self.previous;self.temp.cleanup()

    def cmd(self,*args):
        return subprocess.run(args,cwd=self.root,check=True,text=True,capture_output=True)

    def snapshot(self):
        return {name:(self.root/name).read_bytes() for name in ["Cargo.toml","Cargo.lock","CHANGELOG.md"]}

    def test_dry_run_never_mutates(self):
        before=self.snapshot();result=release.bump("minor",dry_run=True)
        self.assertEqual(result["new"],"0.2.0");self.assertEqual(before,self.snapshot())
        self.assertEqual(self.cmd("git","status","--porcelain").stdout,"")

    def test_bump_changes_app_and_lock_but_keeps_library_pin(self):
        release.bump("patch")
        manifest=tomllib.loads((self.root/"Cargo.toml").read_text())
        self.assertEqual(manifest["package"]["version"],"0.1.1")
        self.assertEqual(manifest["dependencies"]["zgpui"]["version"],"=0.1.1")
        lock=tomllib.loads((self.root/"Cargo.lock").read_text())
        self.assertEqual({p["name"]:p["version"] for p in lock["package"]},{"zcall-gui":"0.1.1","zgpui":"0.1.1"})
        self.assertIn("## 0.1.1",(self.root/"CHANGELOG.md").read_text())

    def test_existing_tag_is_rejected_before_writes(self):
        self.cmd("git","tag","v0.1.1");before=self.snapshot()
        with self.assertRaisesRegex(ValueError,"already exists"):release.bump("patch")
        self.assertEqual(before,self.snapshot())

    def test_failed_lock_generation_rolls_back_exact_bytes(self):
        before=self.snapshot();original=release.run
        def fail(*args,**kwargs):
            if args[:2]==("cargo","generate-lockfile"):
                (self.root/"Cargo.lock").write_text("incomplete\n")
                raise subprocess.CalledProcessError(1,args)
            return original(*args,**kwargs)
        with patch.object(release,"run",side_effect=fail):
            with self.assertRaises(subprocess.CalledProcessError):release.bump("patch")
        self.assertEqual(before,self.snapshot())
        self.assertEqual(self.cmd("git","status","--porcelain").stdout,"")

    def test_exact_ref_rejects_other_commit_and_dirty_tree(self):
        self.cmd("git","tag","v0.1.0")
        self.assertEqual(release.exact("v0.1.0"),"v0.1.0")
        (self.root/"notes").write_text("change\n")
        with self.assertRaisesRegex(ValueError,"worktree"):release.exact("v0.1.0")
        self.cmd("git","add","notes");self.cmd("git","commit","-qm","Later commit")
        with self.assertRaisesRegex(ValueError,"immutable"):release.exact("v0.1.0")

    def test_candidate_requires_the_exact_full_commit(self):
        head=self.cmd("git","rev-parse","HEAD").stdout.strip()
        self.assertEqual(release.candidate(head),head)
        for invalid in ["main",head[:12],"0"*40]:
            with self.subTest(ref=invalid):
                with self.assertRaises(ValueError):release.candidate(invalid)

    def test_custom_versions_are_stable_and_monotonic(self):
        for version in ["0.1.0","0.0.9","01.2.3","v1.2.3","1.0.0-rc.1","../../tag"]:
            with self.subTest(version=version):
                with self.assertRaises(ValueError):release.bump("custom",version,dry_run=True)
        self.assertEqual(release.bump("custom","1.2.3",True)["tag"],"v1.2.3")

class AurMetadata(unittest.TestCase):
    def test_versions_include_epoch_and_package_revision(self):
        def data(version,revision=1,epoch=0):return f"pkgver = {version}\npkgrel = {revision}\nepoch = {epoch}\n"
        self.assertGreater(aur.package_version(data("0.2.0")),aur.package_version(data("0.1.9")))
        self.assertGreater(aur.package_version(data("0.1.0",2)),aur.package_version(data("0.1.0",1)))
        self.assertGreater(aur.package_version(data("0.1.0",1,1)),aur.package_version(data("9.0.0")))

    def test_rendered_metadata_has_real_lines_and_complete_hash_fields(self):
        hashes={"source":"a"*64,"x86_64":"b"*64,"aarch64":"c"*64}
        source=aur.metadata("zcall-gui","0.1.0",hashes)
        binary=aur.metadata("zcall-gui-bin","0.1.0",hashes)
        self.assertGreater(source.count("\n"),15);self.assertNotIn("\\n",source)
        self.assertIn("\tdepends = zcalld>=0.2.0\n",source)
        self.assertIn("\tsha256sums = "+"a"*64+"\n",source)
        for architecture,digest in [("x86_64","b"*64),("aarch64","c"*64)]:
            self.assertIn(f"\tsha256sums_{architecture} = {digest}\n",binary)
        self.assertNotIn("SKIP",binary)

    @unittest.skipUnless(shutil.which("makepkg"),"makepkg is available on the Arch validation host")
    def test_srcinfo_matches_makepkg(self):
        hashes={"source":"a"*64,"x86_64":"b"*64,"aarch64":"c"*64}
        for name in ["zcall-gui","zcall-gui-bin"]:
            with self.subTest(package=name),tempfile.TemporaryDirectory(prefix="zcall-gui-srcinfo-unit-") as directory:
                recipe=(release.ROOT/"packaging/aur"/name/"PKGBUILD.in").read_text()
                for key,value in {"VERSION":"0.1.0","SOURCE_SHA256":hashes["source"],"X86_64_SHA256":hashes["x86_64"],"AARCH64_SHA256":hashes["aarch64"]}.items():recipe=recipe.replace("@"+key+"@",value)
                pathlib.Path(directory,"PKGBUILD").write_text(recipe)
                actual=subprocess.check_output(["makepkg","--printsrcinfo"],cwd=directory,text=True)
                self.assertEqual(actual,aur.metadata(name,"0.1.0",hashes))

    def test_publish_never_downgrades_existing_aur(self):
        with tempfile.TemporaryDirectory(prefix="zcall-gui-aur-test-") as directory:
            root=pathlib.Path(directory);generated=root/"generated";calls=[]
            for name in ["zcall-gui","zcall-gui-bin"]:
                (generated/name).mkdir(parents=True)
                (generated/name/".SRCINFO").write_text("pkgver = 0.1.0\npkgrel = 1\n")
                (generated/name/"PKGBUILD").write_text("would replace a newer release\n")
            def fake_run(args,**kwargs):
                calls.append(args)
                if args[:2]==["git","clone"]:
                    checkout=pathlib.Path(args[-1]);checkout.mkdir(parents=True)
                    (checkout/".SRCINFO").write_text("pkgver = 0.2.0\npkgrel = 2\n")
                else:raise AssertionError("A downgrade attempted a mutation: "+repr(args))
                return subprocess.CompletedProcess(args,0)
            with patch.object(aur,"ROOT",root),patch.object(aur.subprocess,"run",side_effect=fake_run),contextlib.redirect_stdout(io.StringIO()):
                aur.publish(generated)
            self.assertEqual(len(calls),2)
            self.assertTrue(all(args[:2]==["git","clone"] for args in calls))
            for name in ["zcall-gui","zcall-gui-bin"]:
                self.assertIn("0.2.0",(root/"_release/aur"/name/".SRCINFO").read_text())

if __name__=="__main__":unittest.main(verbosity=2)
