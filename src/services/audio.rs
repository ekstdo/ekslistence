use std::{ops::Deref, sync::{Arc, Mutex}};

use pulse::{context::{introspect::{SinkInfo, SourceInfo, SourceOutputInfo}, Context}, mainloop::standard::{IterateResult, Mainloop}, operation::{Operation, State}, proplist::Proplist};
use pulse::context::{FlagSet as ContextFlagSet};
use pulse::stream::{Stream, FlagSet as StreamFlagSet};

pub enum StreamType {
    Microphones, // Source
    App, // Source Input
    Speaker, // Sink
    Recording, // Sink Output
}

pub struct StreamEntry {
    application_id: String,
    description: String,
    is_muted: bool,
    volume: f64,
    icon_name: String,
    id: usize,
    state: String,
    type_: StreamType
}

// impl From<SinkInfo> for StreamEntry {
//     fn from(value: SinkInfo) -> Self {
//         Self {

//         }
//     }
// }

pub struct AudioService {
    mainloop: Arc<Mutex<Mainloop>>,
    context: Arc<Mutex<Context>>,
}

#[derive(Debug, Clone)]
pub enum AudioServiceError {
    NewMainloopError,
    NewContextError,
    ConnectContextError(pulse::error::PAErr),
    IterateError,
    ContextTerminatedError,
}


impl AudioService {
    fn new_context() -> Result<(Arc<Mutex<Mainloop>>, Arc<Mutex<Context>>), AudioServiceError> {
        let mut proplist = Proplist::new().unwrap();
        proplist.set_str(pulse::proplist::properties::APPLICATION_NAME, "FooApp")
            .unwrap();

        let mut mainloop = Mainloop::new().ok_or(AudioServiceError::NewMainloopError)?;

        let mut context = Context::new_with_proplist(
            &mainloop,
            "FooAppContext",
            &proplist
            ).ok_or(AudioServiceError::NewContextError)?;

        context.connect(None, ContextFlagSet::NOFLAGS, None)
            .map_err(AudioServiceError::ConnectContextError)?;

        // Wait for context to be ready
        loop {
            match mainloop.iterate(false) {
                IterateResult::Quit(_) |
                IterateResult::Err(_) => {
                    eprintln!("Iterate state was not success, quitting...");
                    return Err(AudioServiceError::IterateError);
                },
                IterateResult::Success(_) => {},
            }
            match context.get_state() {
                pulse::context::State::Ready => { break; },
                pulse::context::State::Failed |
                pulse::context::State::Terminated => {
                    eprintln!("Context state failed/terminated, quitting...");
                    return Err(AudioServiceError::ContextTerminatedError);
                },
                _ => {},
            }
        }



        Ok((Arc::new(Mutex::new(mainloop)), Arc::new(Mutex::new(context))))
    }

    fn wait_op<T: ?Sized>(&self, op: &Operation<T>) -> Result<(), AudioServiceError> {
        while op.get_state() == State::Running {
            match self.mainloop.lock().unwrap().iterate(true) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    eprintln!("Iterate state was not success, quitting...");
                    return Err(AudioServiceError::IterateError)
                }
                IterateResult::Success(_) => {  }
            }
        }
        Ok(())
    }

    pub fn get_speakers(&self) -> Vec<String> {
        let result = Arc::new(Mutex::new(Vec::new()));
        {
            let w = result.clone();
            let op = self.context.lock().unwrap().introspect().get_source_output_info_list(
                move |x: pulse::callbacks::ListResult<&SourceOutputInfo>| {
                    if let pulse::callbacks::ListResult::Item(e) = x {
                        let name = String::from(e.name.as_ref().unwrap().deref());
                        println!("{:?}", e);
                        w.lock().unwrap().push(name);
                    };
                },
            );
            self.wait_op(&op).unwrap();
        }
        dbg!("hi");
        let a = Arc::into_inner(result).unwrap();
        dbg!("hi");
        let mutex_guard = a.lock().unwrap();
        dbg!("hi");
        Vec::new()
    }

    pub fn new() -> Result<Self, AudioServiceError> {
        let (mainloop, context) = Self::new_context()?;
        Ok(Self {mainloop, context})
    }
}


// speaker: get_sink_info_list
// microphone: get_source_info_list
// apps: get_source_input_info_list
// recorders: get_sink_input_info_list
