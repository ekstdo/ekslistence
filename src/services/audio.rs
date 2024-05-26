use std::{ops::Deref, sync::{Arc, Mutex}};

use pulse::{context::{introspect::{Introspector, SinkInputInfo, SourceOutputInfo}, Context}, def::{SinkState, SourceState}, mainloop::standard::{IterateResult, Mainloop}, operation::{Operation, State}, proplist::Proplist};
use pulse::context::{FlagSet as ContextFlagSet};


#[derive(Clone, Debug)]
pub enum StreamType {
    Microphones(SourceState), // Source
    App(String), // Sink output with application ID
    Speaker(SinkState, u32), // Sink with base volume
    Recording(String), // Source input with optional application name
}


#[derive(Clone, Debug)]
pub struct StreamEntry {
    name: String,
    description: String,
    is_muted: bool,
    volume: Vec<u32>,
    icon_name: String,
    type_: StreamType,
    id: u32
}

impl<'a> From<&SinkInfo<'a>> for StreamEntry {
    fn from(value: &SinkInfo) -> Self {
        let base_volume = value.base_volume.0;
        let channel_volumes = value.volume.get().iter().map(|x| x.0).collect::<Vec<_>>();
        let icon_name = value.proplist.get_str("device.icon_name").unwrap_or("audio-card-analog-pci".into());
        Self {
            name: String::from(value.name.as_ref().unwrap().deref()),
            description: String::from(value.description.as_ref().unwrap().deref()),
            is_muted: value.mute,
            type_: StreamType::Speaker(value.state, base_volume),
            volume: channel_volumes,
            icon_name,
            id: value.index
        }
    }
}

impl<'a> From<&SinkInputInfo<'a>> for StreamEntry {
    fn from(value: &SinkInputInfo) -> Self {
        let channel_volumes = value.volume.get().iter().map(|x| x.0).collect::<Vec<_>>();
        let icon_name = value.proplist.get_str("device.icon_name").unwrap_or("audio-card-analog-pci".into());
        let appname = value.proplist.get_str("application.name");
        let name = String::from(value.name.as_ref().unwrap().deref());
        Self {
            description: name.clone(),
            name,
            is_muted: value.mute,
            type_: StreamType::App(appname.unwrap_or("unknown".into())),
            volume: channel_volumes,
            icon_name,
            id: value.index
        }
    }
}

impl<'a> From<&SourceInfo<'a>> for StreamEntry {
    fn from(value: &SourceInfo) -> Self {
        let base_volume = value.base_volume.0;
        let channel_volumes = value.volume.get().iter().map(|x| x.0).collect::<Vec<_>>();
        let icon_name = value.proplist.get_str("device.icon_name").unwrap_or("audio-card-analog-pci".into());
        Self {
            name: String::from(value.name.as_ref().unwrap().deref()),
            description: String::from(value.description.as_ref().unwrap().deref()),
            is_muted: value.mute,
            type_: StreamType::Microphones(value.state),
            volume: channel_volumes,
            icon_name,
            id: value.index
        }
    }
}

impl<'a> From<&SourceOutputInfo<'a>> for StreamEntry {
    fn from(value: &SourceOutputInfo) -> Self {
        let channel_volumes = value.volume.get().iter().map(|x| x.0).collect::<Vec<_>>();
        let icon_name = value.proplist.get_str("device.icon_name").unwrap_or("record".into());
        let appname = value.proplist.get_str("application.name");
        let name = String::from(value.name.as_ref().unwrap().deref());
        let description = value.proplist.get_str("device.description").unwrap_or(name.clone());
        Self {
            name,
            description,
            is_muted: value.mute,
            type_: StreamType::Recording(appname.unwrap_or("unknown".into())),
            volume: channel_volumes,
            icon_name,
            id: value.index
        }
    }
}

pub struct AudioService {
    mainloop: Rc<RefCell<Mainloop>>,
    context: Rc<RefCell<Context>>,
}

#[derive(Debug, Clone)]
pub enum AudioServiceError {
    NewMainloopError,
    NewContextError,
    ConnectContextError(pulse::error::PAErr),
    IterateError,
    ContextTerminatedError,
}


use pulse::context::introspect::{ServerInfo, SinkInfo, SourceInfo};
use std::cell::RefCell;
use std::rc::Rc;



impl AudioService {
    fn new_context() -> Result<(Rc<RefCell<Mainloop>>, Rc<RefCell<Context>>), AudioServiceError> {
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



        Ok((Rc::new(RefCell::new(mainloop)), Rc::new(RefCell::new(context))))
    }

    fn wait_op<T: ?Sized>(&self, op: &Operation<T>) -> Result<(), AudioServiceError> {
        while op.get_state() == State::Running {
            match self.mainloop.borrow_mut().iterate(true) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    eprintln!("Iterate state was not success, quitting...");
                    return Err(AudioServiceError::IterateError)
                }
                IterateResult::Success(_) => {  }
            }
        }
        Ok(())
    }

    pub fn get<G: ?Sized, F: FnOnce(Introspector, Rc<RefCell<Vec<StreamEntry>>>) -> Operation<G>>(&self, getter: F)  -> Result<Vec<StreamEntry>, AudioServiceError> {
        let result = Rc::new(RefCell::new(Vec::new()));
        {
            let w = result.clone();
            let op = getter(self.context.borrow_mut().introspect(), w);

            self.wait_op(&op)?;
        }
        
        Ok(Rc::try_unwrap(result).unwrap().into_inner())
    }

    pub fn get_speakers(&self) -> Result<Vec<StreamEntry>, AudioServiceError> {
        self.get(|x, w|
            x.get_sink_info_list(
            move |x| {
                if let pulse::callbacks::ListResult::Item(e) = x {
                    w.borrow_mut().push(e.into());
                };
            },
        ))
    }


    pub fn get_microphones(&self) -> Result<Vec<StreamEntry>, AudioServiceError> {
        self.get(|x, w|
            x.get_source_info_list(
            move |x| {
                if let pulse::callbacks::ListResult::Item(e) = x {
                    if e.proplist.get_str("device.class") == Some("sound".to_string()) {
                        w.borrow_mut().push(e.into());
                    }
                };
            },
        ))
    }


    pub fn get_recorders(&self) -> Result<Vec<StreamEntry>, AudioServiceError> {
        self.get(|x, w|
            x.get_source_output_info_list(
            move |x| {
                println!("{:?}", x);
                if let pulse::callbacks::ListResult::Item(e) = x {
                    println!("{:?}", e);
                    // w.borrow_mut().push(e.into());
                };
            },
        ))
    }

    pub fn get_applications(&self) -> Result<Vec<StreamEntry>, AudioServiceError> {
        self.get(|x, w|
            x.get_sink_input_info_list(
            move |x| {
                println!("{:?}", x);
                if let pulse::callbacks::ListResult::Item(e) = x {
                    println!("{:?}", e);
                    // w.borrow_mut().push(e.into());
                };
            },
        ))
    }

    pub fn get_defaults(&self) -> Result<(String, String), AudioServiceError> {
        let sink_source = Rc::new(RefCell::new((String::new(), String::new())));


        {
            let sink_source_clone = sink_source.clone();
            let op = self
                .context
                .borrow()
                .introspect()
                .get_server_info(move |x: &ServerInfo| {
                    let source_name = x.default_source_name.as_ref().unwrap().deref();
                    let sink_name = x.default_sink_name.as_ref().unwrap().deref();
                    *sink_source_clone.borrow_mut() = (String::from(source_name), String::from(sink_name));
                });
            self.wait_op(&op);
        }

        Ok(Rc::try_unwrap(sink_source).unwrap().into_inner())
    }

    pub fn set_microphone(&self, mic: &str) -> Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().set_default_source(mic, |_| ());
        self.wait_op(&op)
    }

    pub fn set_speaker(&self, speaker: &str) -> Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().set_default_sink(speaker, |_| ());
        self.wait_op(&op)
    }

    pub fn set_mute_microphone(&self, mic: &str, yes: bool) ->  Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().introspect().set_source_mute_by_name(mic, yes, None);
        self.wait_op(&op)
    }

    pub fn set_mute_speaker(&self, speaker: &str, yes: bool) ->  Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().introspect().set_sink_mute_by_name(speaker, yes, None);
        self.wait_op(&op)
    }

    pub fn set_mute_application(&self, app: u32, yes: bool) ->  Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().introspect().set_sink_input_mute(app, yes, None);
        self.wait_op(&op)
    }


    pub fn set_mute_recorder(&self, rec: u32, yes: bool) ->  Result<(), AudioServiceError> {
        let op = self.context.borrow_mut().introspect().set_source_output_mute(rec, yes, None);
        self.wait_op(&op)
    }


    pub fn new() -> Result<Self, AudioServiceError> {
        let (mainloop, context) = Self::new_context()?;
        Ok(Self {mainloop, context})
    }
}

impl Drop for AudioService {
    fn drop(&mut self) {
        self.context.borrow_mut().disconnect();
    }
}


// speaker: get_sink_info_list
// microphone: get_source_info_list
// apps: get_source_input_info_list
// recorders: get_sink_input_info_list
