//! Text Controller module.
//!
//! Facade over filesystem API or module text API for text editor. Does discerning between Luna
//! module file and plain text file. In case of luna module idmap and metadata are hidden for the
//! user.

use crate::prelude::*;

use crate::controller;

use failure::_core::fmt::{Formatter, Error};
use flo_stream::{Publisher, Subscriber};
use flo_stream::MessagePublisher;
use file_manager_client as fmc;
use json_rpc::error::RpcError;
use shapely::shared;



// ====================
// === Notification ===
// ====================

/// A buffer size for notification publisher.
///
/// We don't expect much traffic on file notifications, therefore there is no need for setting big
/// buffers.
const NOTIFICATION_BUFFER_SIZE : usize = 36;

/// A notification from TextController.
#[derive(Clone,Debug)]
pub enum Notification {
    /// File contents needs to be set to the following due to synchronization with external state.
    SetNewContent(String),
}



// =======================
// === Text Controller ===
// =======================

/// A handle for file.
///
/// This makes distinction between module and plain text files. The module files are handled by
/// Module Controller, the plain text files are handled directly by File Manager Client.
#[derive(Clone,Debug)]
enum FileHandle {
    PlainText {path:fmc::Path, file_manager:fmc::Handle},
    Module    {controller:controller::module::Handle},
}


shared! { Handle

    /// Data stored by the text controller.
    pub struct State {
        file: FileHandle,
        /// Sink where we put events to be consumed by the view.
        notification_publisher: Publisher<Notification>,
    }

    impl {
        /// Get subscriber receiving controller's notifications.
        pub fn subscribe(&mut self) -> Subscriber<Notification> {
            self.notification_publisher.subscribe()
        }

        /// Get clone of file path handled by this controller.
        pub fn file_path_clone(&self) -> fmc::Path {
            match &self.file {
                FileHandle::PlainText{path,..} => path.clone(),
                FileHandle::Module{controller} => controller.location_as_path(),
            }
        }
    }
}

impl Handle {
    /// Create controller managing plain text file.
    pub fn new_for_plain_test(path:fmc::Path, file_manager:fmc::Handle) -> Self {
        Self::new(FileHandle::PlainText {path,file_manager})
    }
    /// Create controller managing Luna module file.
    pub fn new_for_module(controller:controller::module::Handle) -> Self {
        Self::new(FileHandle::Module {controller})
    }

    /// Read file's content.
    pub fn read_content(&self) -> impl Future<Output=Result<String,RpcError>> {
        match self.file_handle() {
            FileHandle::PlainText {path,mut file_manager} => file_manager.read(path),
            FileHandle::Module {..}               => unimplemented!(),
        }
    }

    /// Store the given content to file.
    pub fn store_content(&self, content:String) -> impl Future<Output=Result<(),RpcError>> {
        match self.file_handle() {
            FileHandle::PlainText {path,mut file_manager} => file_manager.write(path,content),
            FileHandle::Module {..}               => unimplemented!(),
        }
    }

    #[cfg(test)]
    /// Get FileManagerClient handle used by this controller.
    pub fn file_manager(&self) -> fmc::Handle {
        self.with_borrowed(|state| {
            match &state.file {
                FileHandle::PlainText {file_manager,..} => file_manager.clone_ref(),
                FileHandle::Module {..} =>
                    panic!("Cannot get FileManagerHandle from module file"),
            }
        })
    }
}


// === Private functions ===

impl Handle {
    /// Create controller managing plain text file.
    fn new(file_handle:FileHandle) -> Self {
        let state = State {
            file                   : file_handle,
            notification_publisher : Publisher::new(NOTIFICATION_BUFFER_SIZE),
        };
        Self {rc:Rc::new(RefCell::new(state))}
    }

    fn file_handle(&self) -> FileHandle {
        self.with_borrowed(|state| state.file.clone())
    }
}


// === Debug implementations ===

impl Debug for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f,"Text Controller on {:?} }}",self.file)
    }
}

// TODO[ao] why the shared macro cannot derive this?
impl Debug for Handle {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        self.rc.borrow().fmt(f)
    }
}
