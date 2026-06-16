use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct FakeCardState {
    pub(super) status_responses: VecDeque<Result<PcscCardStatus, i32>>,
    pub(super) attributes: HashMap<PcscAttribute, Result<Vec<u8>, i32>>,
    pub(super) transmit_responses: VecDeque<Result<Vec<u8>, i32>>,
    pub(super) control_responses: VecDeque<Result<Vec<u8>, i32>>,
    pub(super) reconnect_calls: Vec<(PcscShareMode, PcscProtocols, PcscDisposition)>,
}

#[derive(Clone)]
pub(super) struct FakePcscCard {
    pub(super) state: Arc<Mutex<FakeCardState>>,
}

impl FakePcscCard {
    pub(super) fn new(state: FakeCardState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }
}

impl PcscCard for FakePcscCard {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32> {
        self.state.lock().unwrap().reconnect_calls.push((
            share_mode,
            preferred_protocols,
            disposition,
        ));
        Ok(())
    }

    fn status2_owned(&self) -> Result<PcscCardStatus, i32> {
        self.state
            .lock()
            .unwrap()
            .status_responses
            .pop_front()
            .unwrap_or(Ok(PcscCardStatus {
                present: true,
                atr: Vec::new(),
                protocol: Some(PcscProtocol::T0),
            }))
    }

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .attributes
            .get(&attribute)
            .cloned()
            .unwrap_or(Ok(Vec::new()))
    }

    fn transmit(&self, _send_buffer: &[u8], _receive_capacity: usize) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .transmit_responses
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }

    fn control(
        &self,
        _control_code: u64,
        _send_buffer: &[u8],
        _receive_capacity: usize,
    ) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .control_responses
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }
}

#[derive(Default)]
pub(super) struct FakePcscBackend {
    readers: Vec<String>,
    cards: HashMap<String, Arc<Mutex<FakeCardState>>>,
}

impl FakePcscBackend {
    pub(super) fn with_reader(mut self, reader: &str, state: FakeCardState) -> Self {
        self.readers.push(reader.to_string());
        self.cards
            .insert(reader.to_string(), Arc::new(Mutex::new(state)));
        self
    }
}

impl PcscBackend for FakePcscBackend {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32> {
        Ok(self.readers.clone())
    }

    fn connect(
        &self,
        reader: &str,
        _share_mode: PcscShareMode,
        _preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32> {
        let state = self.cards.get(reader).cloned().ok_or(NFC_ENOTSUCHDEV)?;
        Ok(Box::new(FakePcscCard { state }))
    }
}
