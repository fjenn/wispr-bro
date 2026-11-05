// SPDX-License-Identifier: GPL-3.0-or-later
//! The local test page for the browsers: a file the tool writes into the
//! results folder and opens with `open -a <browser> <file>`. No network:
//! the page has no external resources. Its own script walks through the
//! fields (focus, select, blur) and announces each step in the window title
//! (`WB-RS04 r1 03 textarea text`), which is the ground truth the tool
//! reads back through AX. The owner does not click anything.

use std::path::{Path, PathBuf};

use crate::apps::Expect;

pub const FILE_NAME: &str = crate::ax::probe::TEST_PAGE_NAME;
/// Milliseconds per step; the tool probes each step within it.
pub const STEP_MS: u32 = 3000;
pub const ROUNDS: u32 = 2;

/// (field name, what the step does, expectation, selected text).
pub const STEPS: &[(&str, Expect, Option<&str>)] = &[
    ("input", Expect::Text, None),
    ("input-select", Expect::Select, Some("brown fox")),
    ("textarea", Expect::Text, None),
    ("textarea-select", Expect::Select, Some("lazy dog")),
    ("editable", Expect::Text, None),
    ("editable-select", Expect::Select, Some("sunny")),
    ("search", Expect::Text, None),
    ("email", Expect::Text, None),
    ("password", Expect::Password, None),
    ("button", Expect::NotText, None),
    ("link", Expect::NotText, None),
    ("body", Expect::NotText, None),
    ("readonly", Expect::Info, None),
    ("dropdown", Expect::NotText, None),
];

/// Steps whose field is a scratch field for the `AXSelectedText` write test.
pub fn set_test_step(name: &str, round: u32) -> bool {
    round == 1 && matches!(name, "input" | "textarea" | "editable")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub round: u32,
    pub n: u32,
    pub name: String,
    pub expect: Expect,
    pub selection: Option<&'static str>,
}

/// `WB-RS04 r1 03 textarea text` → the step. `WB-RS04 done` → None.
pub fn parse_title(title: &str) -> Option<Step> {
    let mut it = title.split_whitespace();
    if it.next()? != "WB-RS04" {
        return None;
    }
    let round = it.next()?.strip_prefix('r')?.parse().ok()?;
    let n: u32 = it.next()?.parse().ok()?;
    let name = it.next()?.to_string();
    let (_, expect, selection) = STEPS.iter().find(|(s, _, _)| *s == name)?;
    Some(Step {
        round,
        n,
        name,
        expect: *expect,
        selection: *selection,
    })
}

pub fn html() -> String {
    let steps: Vec<String> = STEPS
        .iter()
        .map(|(name, e, sel)| {
            format!(
                "[\"{name}\",\"{}\",{}]",
                e.label(),
                sel.map_or("null".to_string(), |s| format!("\"{s}\""))
            )
        })
        .collect();
    PAGE.replace("__STEPS__", &steps.join(","))
        .replace("__STEP_MS__", &STEP_MS.to_string())
        .replace("__ROUNDS__", &ROUNDS.to_string())
}

pub fn write(dir: &Path) -> std::io::Result<PathBuf> {
    let path = dir.join(FILE_NAME);
    std::fs::write(&path, html())?;
    Ok(path)
}

const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>WB-RS04 waiting</title>
<style>
 body{font:15px -apple-system,sans-serif;margin:24px;max-width:640px;background:#fff;color:#111}
 label{display:block;margin:10px 0 2px;color:#555}
 input,textarea,select,[contenteditable]{font:inherit;padding:6px;border:1px solid #888;border-radius:4px;width:100%;box-sizing:border-box}
 [contenteditable]{min-height:2em}
 #status{position:sticky;top:0;background:#ffe;padding:8px;border:1px solid #cc9;margin-bottom:12px}
</style></head>
<body>
<div id="status">Wispr Bro test page (RS-04). Please don't click: the page moves through the fields by itself.</div>
<label for="f-input">Text field</label><input id="f-input" value="The quick brown fox">
<label for="f-textarea">Text area</label><textarea id="f-textarea" rows="3">Jumps over the lazy dog
and naps in the sun.</textarea>
<label>Editable box</label><div id="f-editable" contenteditable="true">A sunny day for testing.</div>
<label for="f-search">Search field</label><input id="f-search" type="search" value="search me">
<label for="f-email">E-mail field</label><input id="f-email" type="email" value="someone">
<label for="f-password">Password field</label><input id="f-password" type="password" value="secret123">
<label for="f-readonly">Read-only field</label><input id="f-readonly" readonly value="cannot type here">
<label for="f-dropdown">Drop-down</label><select id="f-dropdown"><option>One</option><option>Two</option></select>
<p><button id="f-button">A button</button> <a id="f-link" href="#nowhere">A link</a></p>
<script>
const STEPS=[__STEPS__], STEP_MS=__STEP_MS__, ROUNDS=__ROUNDS__;
const $=id=>document.getElementById(id);
function caretEnd(el){const r=document.createRange();r.selectNodeContents(el);r.collapse(false);const s=getSelection();s.removeAllRanges();s.addRange(r);}
function selectWord(el,word){
  if(el.setSelectionRange){const i=el.value.indexOf(word);el.focus();el.setSelectionRange(i,i+word.length);return;}
  const t=el.firstChild,i=t.textContent.indexOf(word),r=document.createRange();r.setStart(t,i);r.setEnd(t,i+word.length);
  el.focus();const s=getSelection();s.removeAllRanges();s.addRange(r);}
function run(name,sel){
  const base=name.replace("-select","");
  if(name==="body"){if(document.activeElement)document.activeElement.blur();getSelection().removeAllRanges();return;}
  const el=$("f-"+base);
  if(sel){selectWord(el,sel);return;}
  el.focus();
  if(el.setSelectionRange&&el.type!=="email"){try{const n=el.value.length;el.setSelectionRange(n,n);}catch(e){}}
  else if(el.isContentEditable){caretEnd(el);}
}
let i=0;
function tick(){
  const total=STEPS.length*ROUNDS;
  if(i>=total){document.title="WB-RS04 done";$("status").textContent="Done. You can close this tab.";if(document.activeElement)document.activeElement.blur();return;}
  const round=Math.floor(i/STEPS.length)+1,[name,expect,sel]=STEPS[i%STEPS.length],n=String(i%STEPS.length+1).padStart(2,"0");
  run(name,sel);
  document.title="WB-RS04 r"+round+" "+n+" "+name+" "+expect;
  $("status").textContent="Step "+(i+1)+" of "+total+": "+name+". Please don't click.";
  i++;setTimeout(tick,STEP_MS);
}
// Safari leaves focus in the address bar after `open`: start anyway after 5 s.
const loaded=Date.now();
function start(){if(document.hasFocus()||Date.now()-loaded>5000){setTimeout(tick,3000);}else{setTimeout(start,250);}}
start();
</script>
</body></html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_round_trip() {
        let s = parse_title("WB-RS04 r2 04 textarea-select select").unwrap();
        assert_eq!((s.round, s.n), (2, 4));
        assert_eq!(s.expect, Expect::Select);
        assert_eq!(s.selection, Some("lazy dog"));
        assert_eq!(parse_title("WB-RS04 done"), None);
        assert_eq!(parse_title("WB-RS04 waiting"), None);
        assert_eq!(parse_title("Inbox (3)"), None);
        assert_eq!(
            parse_title("WB-RS04 r1 09 password password")
                .unwrap()
                .expect,
            Expect::Password
        );
    }

    #[test]
    fn page_is_self_contained() {
        let h = html();
        assert!(!h.contains("__STEPS__"));
        assert!(!h.contains("http"), "no external resources");
        assert!(!h.contains("src="));
        for (name, _, sel) in STEPS {
            let base = name.trim_end_matches("-select");
            if *name != "body" {
                assert!(h.contains(&format!("id=\"f-{base}\"")), "{base}");
            }
            if let Some(s) = sel {
                assert!(h.contains(s), "{s} is in the page");
            }
        }
    }

    #[test]
    fn set_test_only_in_round_one_scratch_fields() {
        assert!(set_test_step("textarea", 1));
        assert!(!set_test_step("textarea", 2));
        assert!(!set_test_step("password", 1));
    }
}
