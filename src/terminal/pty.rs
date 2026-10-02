use std::{collections::HashMap, path::PathBuf, sync::Arc};

use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, Notifier},
    sync::FairMutex,
    term::{Config, Term, test::TermSize},
    tty::{self, Options, Shell},
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// Hands a terminal's events to its view.
#[derive(Clone)]
pub(crate) struct Listener(UnboundedSender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if self.0.unbounded_send(event).is_err() {
            log::debug!("terminal: an event after its view closed");
        }
    }
}

/// What a terminal runs: a program and its arguments, the login shell when none; where; and variables on top of TERM, COLORTERM, and LANG when the app has none.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub program: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

/// A grid, the channel of its events, and the pseudo-terminal that feeds it when one does.
pub(crate) struct Session {
    pub term: Arc<FairMutex<Term<Listener>>>,
    pub pty: Option<Notifier>,
    pub events: UnboundedReceiver<Event>,
}

/// A grid no process feeds, for replayed output.
pub(crate) fn detached(size: TermSize) -> Session {
    let (sender, events) = unbounded();
    let term = Term::new(Config::default(), &size, Listener(sender));
    Session {
        term: Arc::new(FairMutex::new(term)),
        pty: None,
        events,
    }
}

/// Starts `launch` on a pseudo-terminal of `size` cells, each `cell` pixels.
pub(crate) fn spawn(launch: &Launch, size: TermSize, cell: (u16, u16)) -> anyhow::Result<Session> {
    if let Some(cwd) = &launch.cwd {
        anyhow::ensure!(
            cwd.is_dir(),
            "a terminal cannot start in {}: no such folder",
            cwd.display()
        );
        #[cfg(unix)]
        rustix::fs::access(cwd, rustix::fs::Access::EXEC_OK).map_err(|error| {
            anyhow::anyhow!("a terminal cannot enter {}: {error}", cwd.display())
        })?;
    }
    let (sender, events) = unbounded();
    let listener = Listener(sender);
    let term = Arc::new(FairMutex::new(Term::new(
        Config::default(),
        &size,
        listener.clone(),
    )));
    let mut env: HashMap<String, String> = HashMap::from([
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
    ]);
    if std::env::var_os("LANG").is_none() {
        env.insert("LANG".into(), "en_US.UTF-8".into());
    }
    env.extend(launch.env.iter().cloned());
    let options = Options {
        shell: launch
            .program
            .clone()
            .map(|(program, args)| Shell::new(program, args)),
        working_directory: launch.cwd.clone(),
        drain_on_exit: true,
        env,
        #[cfg(target_os = "windows")]
        escape_args: true,
    };
    let pty = tty::new(&options, window_size(&size, cell), 0)?;
    let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)?;
    let pty = Notifier(event_loop.channel());
    event_loop.spawn();
    log::info!("terminal: started {:?}", launch.program);
    Ok(Session {
        term,
        pty: Some(pty),
        events,
    })
}

pub(crate) fn window_size(size: &TermSize, cell: (u16, u16)) -> WindowSize {
    let cells =
        |count: usize| u16::try_from(count).expect("a terminal is under 65536 cells a side");
    WindowSize {
        num_lines: cells(size.screen_lines),
        num_cols: cells(size.columns),
        cell_width: cell.0,
        cell_height: cell.1,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use alacritty_terminal::index::{Column, Line};
    use futures::StreamExt;

    use super::*;

    #[test]
    fn a_folder_that_cannot_be_entered_stops_the_start() {
        use std::os::unix::fs::PermissionsExt;
        let folder = std::env::temp_dir().join(format!("ely-shut-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a temp folder");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o600))
            .expect("mode 600");
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), "true".into()])),
            cwd: Some(folder.clone()),
            ..Launch::default()
        };
        let started = spawn(&launch, TermSize::new(20, 4), (8, 16));
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
            .expect("mode 700");
        std::fs::remove_dir(&folder).expect("the temp folder goes");
        let error = started.err().expect("a folder it cannot enter is an error");
        assert!(error.to_string().contains("ely-shut"));
    }

    #[test]
    fn a_missing_folder_stops_the_start() {
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), "true".into()])),
            cwd: Some("/nonexistent/ely".into()),
            ..Launch::default()
        };
        let error = spawn(&launch, TermSize::new(20, 4), (8, 16))
            .err()
            .expect("a missing folder is an error");
        assert!(error.to_string().contains("/nonexistent/ely"));
    }

    #[test]
    fn a_program_prints_into_the_grid_through_a_pseudo_terminal() {
        let launch = Launch {
            program: Some((
                "/bin/sh".into(),
                vec!["-c".into(), "printf 'ely %s' \"$ELY\"".into()],
            )),
            env: vec![("ELY".into(), "ok".into())],
            ..Launch::default()
        };
        let mut session = spawn(&launch, TermSize::new(20, 4), (8, 16)).expect("sh starts");
        let status = futures::executor::block_on(async {
            let mut status = None;
            while let Some(event) = session.events.next().await {
                match event {
                    Event::ChildExit(ended) => status = Some(ended),
                    Event::Exit => break,
                    _ => {}
                }
            }
            status
        });
        assert!(status.expect("the program ended").success());
        let term = session.term.lock();
        let row = &term.grid()[Line(0)];
        let text: String = (0..6).map(|column| row[Column(column)].c).collect();
        assert_eq!(text, "ely ok");
    }
}
