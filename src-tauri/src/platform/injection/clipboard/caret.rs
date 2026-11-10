//! The character before the caret in the focused control, via UI Automation.
//!
//! Best effort: plenty of controls (and apps) expose no text pattern, and a
//! hung app can stall the query. Every failure reads as [`Before::Unknown`].

use std::time::Duration;

use windows::core::Interface;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, TextPatternRangeEndpoint_End,
    TextPatternRangeEndpoint_Start, TextUnit_Character, UIA_TextPatternId,
};

use super::spacing::Before;

/// Longest the paste waits for the target to answer.
const QUERY_TIMEOUT: Duration = Duration::from_millis(150);

/// Ask the focused control what precedes its caret. A query that overruns
/// [`QUERY_TIMEOUT`] is abandoned: its thread ends whenever the target answers.
pub async fn before_caret() -> Before {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("caret-query".into())
        .spawn(move || {
            let _ = sender.send(query());
        });
    if spawned.is_err() {
        return Before::Unknown;
    }
    match tokio::time::timeout(QUERY_TIMEOUT, receiver).await {
        Ok(Ok(before)) => before,
        _ => Before::Unknown,
    }
}

/// Balances `CoInitializeEx` on every exit.
struct ComScope;

impl ComScope {
    fn enter() -> Option<Self> {
        // SAFETY: paired with the `CoUninitialize` in `drop`.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .is_ok()
            .then_some(Self)
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        // SAFETY: only constructed after a successful `CoInitializeEx`.
        unsafe { CoUninitialize() };
    }
}

fn query() -> Before {
    let Some(_com) = ComScope::enter() else {
        return Before::Unknown;
    };
    read_previous_char().unwrap_or(Before::Unknown)
}

fn read_previous_char() -> windows::core::Result<Before> {
    // SAFETY: COM is initialised on this thread by `ComScope`; every interface
    // is released when dropped.
    unsafe {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
        let pattern: IUIAutomationTextPattern = automation
            .GetFocusedElement()?
            .GetCurrentPattern(UIA_TextPatternId)?
            .cast()?;
        let selections = pattern.GetSelection()?;
        if selections.Length()? == 0 {
            return Ok(Before::Unknown);
        }
        // A selection is replaced by the paste, so what matters is what
        // precedes its start: collapse to it, then step one character back.
        let range = selections.GetElement(0)?.Clone()?;
        range.MoveEndpointByRange(
            TextPatternRangeEndpoint_End,
            &range,
            TextPatternRangeEndpoint_Start,
        )?;
        if range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1)? == 0 {
            return Ok(Before::Start);
        }
        Ok(range
            .GetText(1)?
            .to_string()
            .chars()
            .next()
            .map_or(Before::Unknown, Before::Char))
    }
}
