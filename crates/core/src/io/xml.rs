//! A small XML reader for exchange files: elements and text only (attributes are skipped).

#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub text: String,
    pub kids: Vec<Node>,
}

impl Node {
    pub fn get(&self, name: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.kids.iter().filter(move |k| k.name == name)
    }
    /// Trimmed text of a child element, or "".
    pub fn val(&self, name: &str) -> &str {
        self.get(name).map_or("", |k| k.text.trim())
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find(';') else { out.push_str(&rest[i..]); return out };
        let ent = &rest[i + 1..i + j];
        let c = match ent {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c { Some(c) => out.push(c), None => out.push_str(&rest[i..i + j + 1]) }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Parses a document and returns its root element.
pub fn parse(s: &str) -> Result<Node, String> {
    let mut stack = vec![Node::default()];
    let mut rest = s;
    let bad = || "The XML file is damaged.".to_string();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('<') {
            let skip = |r: &str, end: &str| r.find(end).map(|k| k + end.len()).ok_or_else(bad);
            if r.starts_with('?') {
                rest = &r[skip(r, "?>")?..];
            } else if let Some(c) = r.strip_prefix("!--") {
                rest = &c[skip(c, "-->")?..];
            } else if let Some(c) = r.strip_prefix("![CDATA[") {
                let k = c.find("]]>").ok_or_else(bad)?;
                stack.last_mut().unwrap().text.push_str(&c[..k]);
                rest = &c[k + 3..];
            } else if r.starts_with('!') {
                rest = &r[skip(r, ">")?..];
            } else if let Some(c) = r.strip_prefix('/') {
                rest = &c[skip(c, ">")?..];
                let node = stack.pop().filter(|_| !stack.is_empty()).ok_or_else(bad)?;
                stack.last_mut().ok_or_else(bad)?.kids.push(node);
            } else {
                let mut q = None;
                let end = r.char_indices().find(|&(_, ch)| match q { Some(x) => { if ch == x { q = None } false } None => { if ch == '"' || ch == '\'' { q = Some(ch) } ch == '>' } }).map(|(k, _)| k).ok_or_else(bad)?;
                let tag = &r[..end];
                let name = tag.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
                let name = name.rsplit(':').next().unwrap_or(name).to_string();
                // Exchange files nest a few levels; a deep tree would overflow the stack when dropped.
                if stack.len() > 64 { return Err(bad()) }
                let node = Node { name, ..Default::default() };
                if tag.ends_with('/') { stack.last_mut().unwrap().kids.push(node) } else { stack.push(node) }
                rest = &r[end + 1..];
            }
        } else {
            let k = rest.find('<').unwrap_or(rest.len());
            stack.last_mut().unwrap().text.push_str(&unescape(&rest[..k]));
            rest = &rest[k..];
        }
    }
    let mut root = stack.pop().ok_or_else(bad)?;
    root.kids.pop().filter(|_| stack.is_empty()).ok_or_else(bad)
}
