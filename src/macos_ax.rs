use accessibility_ng::{AXAttribute, AXUIElement};
#[cfg(feature = "rich-content")]
use accessibility_sys_ng::kAXRTFForRangeParameterizedAttribute;
use accessibility_sys_ng::{kAXFocusedUIElementAttribute, kAXSelectedTextAttribute};
#[cfg(feature = "rich-content")]
use core_foundation::base::CFType;
#[cfg(feature = "rich-content")]
use core_foundation::data::CFData;
use core_foundation::string::CFString;
use macos_accessibility_client::accessibility::application_is_trusted;
use std::process::Command;

pub(crate) fn get_selected_text_by_ax() -> Result<String, String> {
    let system_element = AXUIElement::system_wide();
    let Some(selected_element) = system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .map(|element| element.downcast_into::<AXUIElement>())
        .ok()
        .flatten()
    else {
        return Err("No focused UI element".to_string());
    };

    let Some(selected_text) = selected_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXSelectedTextAttribute,
        )))
        .map(|text| text.downcast_into::<CFString>())
        .ok()
        .flatten()
    else {
        return Err("No selected text".to_string());
    };

    Ok(selected_text.to_string())
}

#[cfg(feature = "rich-content")]
pub(crate) fn try_selected_rtf_by_ax() -> Option<String> {
    if !application_is_trusted() {
        return None;
    }
    get_selected_rtf_by_ax()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(feature = "rich-content")]
fn get_selected_rtf_by_ax() -> Result<String, String> {
    let system_element = AXUIElement::system_wide();
    let Some(selected_element) = system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .map(|element| element.downcast_into::<AXUIElement>())
        .ok()
        .flatten()
    else {
        return Err("No focused UI element".to_string());
    };

    let selected_range = selected_element
        .attribute(&AXAttribute::selected_text_range())
        .map_err(|_| "No selected text range".to_string())?;
    let range = selected_range
        .get_value::<core_foundation::base::CFRange>()
        .map_err(|_| "Invalid selected text range".to_string())?;
    if range.length <= 0 {
        return Err("No selected text range".to_string());
    }

    let attr = AXAttribute::<CFType>::new(&CFString::from_static_string(
        kAXRTFForRangeParameterizedAttribute,
    ));
    let rtf_data = selected_element
        .parameterized_attribute(&attr, &selected_range)
        .map(|value| value.downcast_into::<CFData>())
        .ok()
        .flatten()
        .ok_or_else(|| "No RTF for selected range".to_string())?;

    // RTF data from the AX API is expected to be valid UTF-8. Using `from_utf8_lossy`
    // would silently replace invalid bytes with U+FFFD, corrupting the RTF payload
    // and causing downstream parse failures. We fail explicitly instead.
    String::from_utf8(rtf_data.bytes().to_vec())
        .map_err(|_| "RTF payload contains non-UTF-8 bytes".to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ClipboardBorrowResult {
    Success(String),
    Empty,
    RestoreFailed,
}

pub(crate) fn run_clipboard_borrow_script() -> Result<ClipboardBorrowResult, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(APPLE_SCRIPT)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let mut lines = stdout.lines();
    match lines.next().unwrap_or_default() {
        "STATUS:OK" => Ok(ClipboardBorrowResult::Success(
            lines.collect::<Vec<_>>().join("\n"),
        )),
        "STATUS:EMPTY" => Ok(ClipboardBorrowResult::Empty),
        "STATUS:RESTORE_FAILED" => Ok(ClipboardBorrowResult::RestoreFailed),
        _ => Ok(ClipboardBorrowResult::Empty),
    }
}

const APPLE_SCRIPT: &str = r#"
use AppleScript version "2.4"
use scripting additions
use framework "Foundation"
use framework "AppKit"

set savedAlertVolume to alert volume of (get volume settings)
set savedClipboard to the clipboard

set thePasteboard to current application's NSPasteboard's generalPasteboard()
set theCount to thePasteboard's changeCount()

tell application "System Events"
    set volume alert volume 0
end tell

tell application "System Events" to keystroke "c" using {command down}
delay 0.12

tell application "System Events"
    set volume alert volume savedAlertVolume
end tell

if thePasteboard's changeCount() is theCount then
    try
        set the clipboard to savedClipboard
        return "STATUS:EMPTY"
    on error
        return "STATUS:RESTORE_FAILED"
    end try
end if

set theSelectedText to the clipboard

try
    set the clipboard to savedClipboard
on error
    return "STATUS:RESTORE_FAILED"
end try

return "STATUS:OK" & linefeed & theSelectedText
"#;
