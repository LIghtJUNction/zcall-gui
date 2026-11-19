use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Json {
    #[default]
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}
static NULL: Json = Json::Null;
impl Json {
    pub fn get(&self, key: &str) -> &Json {
        if let Self::Object(map) = self {
            map.get(key).unwrap_or(&NULL)
        } else {
            &NULL
        }
    }
    pub fn text(&self) -> &str {
        match self {
            Self::String(s) | Self::Number(s) => s,
            _ => "",
        }
    }
    pub fn boolean(&self) -> bool {
        matches!(self, Self::Bool(true))
    }
    pub fn array(&self) -> &[Json] {
        if let Self::Array(items) = self {
            items
        } else {
            &[]
        }
    }
    pub fn object(values: impl IntoIterator<Item = (String, Json)>) -> Self {
        Self::Object(values.into_iter().collect())
    }
}
pub fn parse(text: &str) -> Result<Json, String> {
    if text.len() > 2 * 1024 * 1024 {
        return Err("JSON response exceeds 2 MiB".into());
    }
    let mut parser = Parser { text, at: 0 };
    let value = parser.value(0)?;
    parser.space();
    if parser.at != text.len() {
        return Err("JSON has trailing data".into());
    }
    Ok(value)
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }
    fn consume(&mut self, c: u8) -> bool {
        self.space();
        if self.text.as_bytes().get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > 32 {
            return Err("JSON nesting exceeds 32".into());
        }
        self.space();
        match self.text.as_bytes().get(self.at).copied() {
            Some(b'"') => self.string().map(Json::String),
            Some(b'{') => {
                self.at += 1;
                let mut map = BTreeMap::new();
                if self.consume(b'}') {
                    return Ok(Json::Object(map));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    if !self.consume(b':') {
                        return Err("JSON object missing colon".into());
                    }
                    let value = self.value(depth + 1)?;
                    if map.insert(key, value).is_some() {
                        return Err("JSON duplicate key".into());
                    }
                    if self.consume(b'}') {
                        return Ok(Json::Object(map));
                    }
                    if !self.consume(b',') {
                        return Err("JSON object missing comma".into());
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                if self.consume(b']') {
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.consume(b']') {
                        return Ok(Json::Array(items));
                    }
                    if !self.consume(b',') {
                        return Err("JSON array missing comma".into());
                    }
                }
            }
            Some(b't') if self.text[self.at..].starts_with("true") => {
                self.at += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.text[self.at..].starts_with("false") => {
                self.at += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.text[self.at..].starts_with("null") => {
                self.at += 4;
                Ok(Json::Null)
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.at;
                if self.text.as_bytes()[self.at] == b'-' {
                    self.at += 1;
                }
                match self.text.as_bytes().get(self.at) {
                    Some(b'0') => self.at += 1,
                    Some(b'1'..=b'9') => {
                        while self
                            .text
                            .as_bytes()
                            .get(self.at)
                            .is_some_and(u8::is_ascii_digit)
                        {
                            self.at += 1;
                        }
                    }
                    _ => return Err("Invalid JSON number".into()),
                }
                for marker in *b".e" {
                    let b = self.text.as_bytes().get(self.at).copied();
                    if b == Some(marker) || (marker == b'e' && b == Some(b'E')) {
                        self.at += 1;
                        if marker == b'e'
                            && matches!(self.text.as_bytes().get(self.at), Some(b'+' | b'-'))
                        {
                            self.at += 1;
                        }
                        let digits = self.at;
                        while self
                            .text
                            .as_bytes()
                            .get(self.at)
                            .is_some_and(u8::is_ascii_digit)
                        {
                            self.at += 1;
                        }
                        if digits == self.at {
                            return Err("Invalid JSON fraction/exponent".into());
                        }
                    }
                }
                Ok(Json::Number(self.text[start..self.at].to_owned()))
            }
            _ => Err("Invalid JSON value".into()),
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.at.checked_add(4).ok_or("JSON escape overflow")?;
        let digits = self
            .text
            .get(self.at..end)
            .ok_or("Incomplete JSON escape")?;
        let value = u32::from_str_radix(digits, 16).map_err(|_| "Invalid JSON Unicode escape")?;
        self.at = end;
        Ok(value)
    }
    fn string(&mut self) -> Result<String, String> {
        if !self.consume(b'"') {
            return Err("Expected JSON string".into());
        }
        let mut result = String::new();
        loop {
            let c = self.text[self.at..]
                .chars()
                .next()
                .ok_or("Unterminated JSON string")?;
            self.at += c.len_utf8();
            match c {
                '"' => return Ok(result),
                '\\' => {
                    let escape = *self
                        .text
                        .as_bytes()
                        .get(self.at)
                        .ok_or("Incomplete JSON escape")?;
                    self.at += 1;
                    match escape {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\x08'),
                        b'f' => result.push('\x0c'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'u' => {
                            let high = self.hex4()?;
                            let cp = if (0xd800..=0xdbff).contains(&high) {
                                if !self.text[self.at..].starts_with("\\u") {
                                    return Err("Missing low surrogate".into());
                                }
                                self.at += 2;
                                let low = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err("Invalid low surrogate".into());
                                }
                                0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            } else {
                                high
                            };
                            result.push(char::from_u32(cp).ok_or("Invalid Unicode scalar")?);
                        }
                        _ => return Err("Invalid JSON escape".into()),
                    }
                }
                c if c < ' ' => return Err("Control character in JSON string".into()),
                _ => result.push(c),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_and_surrogates() {
        assert_eq!(
            parse(r#"{"name":"模型 \ud83d\ude80"}"#)
                .unwrap()
                .get("name")
                .text(),
            "模型 🚀"
        );
    }
    #[test]
    fn reject_ambiguous_input() {
        for text in [
            r#"{"x":1,"x":2}"#,
            "[1,]",
            "true false",
            "01",
            "1.",
            "1e",
            r#""\ud800""#,
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
    #[test]
    fn nested_cli_state() {
        let json =
            parse(r#"{"providers":[{"name":"local","configured":true}],"count":1e2}"#).unwrap();
        assert!(json.get("providers").array()[0].get("configured").boolean());
    }
}
