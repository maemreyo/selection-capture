#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinuxSession {
    pub(crate) wayland: bool,
    pub(crate) x11: bool,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
pub(crate) struct LinuxCommandSpec {
    pub(crate) program: &'static str,
    pub(crate) args: &'static [&'static str],
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn detect_linux_session(
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> LinuxSession {
    LinuxSession {
        wayland: wayland_display
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        x11: display
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn clipboard_command_plan(session: LinuxSession) -> &'static [LinuxCommandSpec] {
    const WAYLAND_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
    ];
    const X11_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
    ];
    const MIXED_DEFAULT: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
    ];

    if session.wayland && !session.x11 {
        &WAYLAND_FIRST
    } else if session.x11 && !session.wayland {
        &X11_FIRST
    } else {
        &MIXED_DEFAULT
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn primary_selection_command_plan(
    session: LinuxSession,
) -> &'static [LinuxCommandSpec] {
    const WAYLAND_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
    ];
    const X11_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
    ];
    const MIXED_DEFAULT: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
    ];

    if session.wayland && !session.x11 {
        &WAYLAND_FIRST
    } else if session.x11 && !session.wayland {
        &X11_FIRST
    } else {
        &MIXED_DEFAULT
    }
}
