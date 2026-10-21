//! Sessions panel semantics: menus, tooltips and confirmations (SES-1a, SES-1g, SES-1i).
//! The v0.2.15 tests are React tests (`SessionsPanel.test.tsx`, `App.tsx` flows); these keep
//! their observable intent without the DOM.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use session_library::library::{TreeNode, build_tree};
use session_library::panel::*;
use session_library::*;

fn ssh(id: &str, folder: &str) -> SshSession {
    SshSession {
        id: id.into(),
        name: id.into(),
        folder: folder.into(),
        host: format!("{id}.example.com"),
        port: 22,
        username: "u".into(),
        auth: SavedAuth::Agent,
        jump_ids: vec![],
    }
}

fn labels(items: &[MenuItem]) -> Vec<String> {
    items
        .iter()
        .filter(|i| **i != MenuItem::Separator)
        .map(MenuItem::label)
        .collect()
}

fn library() -> SessionLibrary {
    let entries: Vec<LibrarySession> = vec![
        ssh("web", "Prod").into(),
        ssh("db", "Prod/DB").into(),
        TelnetSession {
            id: "t".into(),
            name: "sw".into(),
            folder: "Lab".into(),
            host: "sw.lan".into(),
            port: 23,
        }
        .into(),
    ];
    entries.into_iter().fold(SessionLibrary::default(), |acc, s| {
        acc.reduce(&LibraryAction::SaveSession(s))
    })
}

#[test]
fn opens_a_session_menu_connect_copy_edit_duplicate_delete() {
    // SessionsPanel.test.tsx: it("opens a session menu on right click: connect, edit, duplicate, delete")
    let menu = session_menu(&ssh("web", "").into(), true);
    assert_eq!(
        labels(&menu),
        ["Connect", "Copy SSH command", "Edit…", "Duplicate", "Delete"]
    );
    assert_eq!(menu[menu.len() - 2], MenuItem::Separator);
}

#[test]
fn offers_copy_ssh_command_for_ssh_sessions_only() {
    let telnet = TelnetSession {
        id: "t".into(),
        name: "sw".into(),
        folder: String::new(),
        host: "h".into(),
        port: 23,
    };
    assert_eq!(
        labels(&session_menu(&telnet.into(), true)),
        ["Connect", "Edit…", "Duplicate", "Delete"]
    );
}

#[test]
fn opens_a_folder_menu_new_session_new_folder_connect_all_delete() {
    // SessionsPanel.test.tsx: it("opens a folder menu on right click: new session, new folder, connect all, delete")
    let lib = library();
    let tree = build_tree(&lib, "");
    let menu = folder_menu(&tree, "Prod", true);
    assert_eq!(
        labels(&menu),
        [
            "New session here…",
            "New folder here…",
            "Connect all (2)",
            "Delete folder"
        ]
    );
    assert_eq!(menu[3], MenuItem::Separator);
}

#[test]
fn connect_all_counts_the_sessions_shown_in_the_folder_and_is_empty_for_empty_folders() {
    let lib = library().reduce(&LibraryAction::AddFolder { path: "Empty".into() });
    let filtered = build_tree(&lib, "db");
    assert!(folder_menu(&filtered, "Prod", true).contains(&MenuItem::ConnectAll { count: 1 }));
    let tree = build_tree(&lib, "");
    assert!(folder_menu(&tree, "Empty", true).contains(&MenuItem::ConnectAll { count: 0 }));
    let prod = find_folder(&tree, "Prod").unwrap();
    let ids: Vec<&str> = sessions_under(prod).iter().map(|s| s.id()).collect();
    assert_eq!(ids, ["db", "web"]);
}

#[test]
fn offers_only_connect_and_copying_while_the_library_cannot_be_edited() {
    // SessionsPanel.test.tsx: it("offers only Connect and copying in the session menu while the library cannot be edited")
    assert_eq!(
        labels(&session_menu(&ssh("web", "").into(), false)),
        ["Connect", "Copy SSH command"]
    );
    let lib = library();
    let tree = build_tree(&lib, "");
    assert_eq!(labels(&folder_menu(&tree, "Prod", false)), ["Connect all (2)"]);
}

#[test]
fn tooltips_say_where_each_session_connects() {
    let lib = library();
    let tip = |s: LibrarySession| session_tooltip(&s, &lib);
    assert_eq!(
        tip(SshSession {
            port: 2222,
            ..ssh("web", "")
        }
        .into()),
        "u@web.example.com:2222"
    );
    let telnet = TelnetSession {
        id: "t".into(),
        name: "sw".into(),
        folder: String::new(),
        host: "sw.lan".into(),
        port: 23,
    };
    assert_eq!(tip(telnet.into()), "telnet sw.lan:23");
    let serial = SerialSession {
        id: "c".into(),
        name: "c".into(),
        folder: String::new(),
        settings: SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::None,
        },
    };
    assert_eq!(tip(serial.into()), "/dev/ttyUSB0 @ 9600");
    let local = |program: Option<&str>, cwd: Option<&str>| LocalSession {
        id: "l".into(),
        name: "l".into(),
        folder: String::new(),
        program: program.map(Into::into),
        args: vec![],
        cwd: cwd.map(Into::into),
    };
    assert_eq!(tip(local(Some("/bin/zsh"), Some("~/src")).into()), "/bin/zsh in ~/src");
    assert_eq!(tip(local(None, None).into()), "System default shell");
    let mosh = |ssh_session_id: &str| MoshSession {
        id: "m".into(),
        name: "m".into(),
        folder: String::new(),
        ssh_session_id: ssh_session_id.into(),
        server_command: None,
    };
    assert_eq!(tip(mosh("web").into()), r#"mosh, logging in with "web""#);
    assert_eq!(tip(mosh("gone").into()), "mosh (its SSH session no longer exists)");
}

#[test]
fn asks_before_deleting_naming_the_sessions_that_use_the_deleted_ones_as_jump_hosts() {
    // App.tsx `confirmDelete` (SES-1i).
    let gateway = ssh("gw", "Gw");
    let lib = SessionLibrary::default().reduce(&LibraryAction::SaveSessions(vec![
        gateway.clone().into(),
        SshSession {
            jump_ids: vec!["gw".into()],
            ..ssh("app", "")
        }
        .into(),
        SshSession {
            jump_ids: vec!["gw".into()],
            ..ssh("api", "")
        }
        .into(),
    ]));
    let delete = LibraryAction::DeleteSession { id: "gw".into() };
    assert_eq!(
        delete_confirmation(&delete_session_question("gw"), &lib, &delete),
        "Delete session \"gw\"?\n\nUsed as a jump host by \"app\", \"api\"; it will be removed from their jump hosts."
    );
    let folder = LibraryAction::DeleteFolder { path: "Lab".into() };
    assert_eq!(
        delete_confirmation(&delete_folder_question("Lab"), &lib, &folder),
        "Delete folder \"Lab\" and every session inside it?"
    );
}

#[test]
fn prompts_for_a_new_folder_and_creates_it_inside_its_parent() {
    assert_eq!(new_folder_prompt(""), "New folder");
    assert_eq!(new_folder_prompt("Prod"), "New folder inside \"Prod\"");
    assert_eq!(
        new_folder_action("Prod", " DB "),
        Some(LibraryAction::AddFolder {
            path: "Prod/ DB ".into()
        })
    );
    assert_eq!(
        new_folder_action("", "A/B"),
        Some(LibraryAction::AddFolder { path: "A/B".into() })
    );
    assert_eq!(new_folder_action("Prod", "  "), None);
    let lib = SessionLibrary::default().reduce(&new_folder_action("Prod", " DB ").unwrap());
    assert_eq!(lib.folders, ["Prod", "Prod/DB"]);
}

#[test]
fn tree_rows_keep_folder_paths() {
    let lib = library();
    let tree = build_tree(&lib, "");
    assert!(matches!(find_folder(&tree, "Prod/DB"), Some(TreeNode::Folder { name, .. }) if name == "DB"));
}
