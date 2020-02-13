//! This module contains all the controllers. They cover everything that is
//! between clients of remote services (like language server and file manager)
//! and views.
//!
//! The controllers create a tree-like structure, with project controller being
//! a root, then module controllers below, then graph/text controller and so on.
//!
//! As a general rule, while the "upper" (i.e. closer to root) nodes may keep
//! handles to the "lower" nodes (e.g. to allow their reuse), they should never
//! manage their lifetime.
//!
//! Primarily views are considered owners of their respective controllers.
//! Additionally, controllers are allowed to keep strong handle "upwards".
//!
//! Controllers store their handles using `utils::cell` handle types to ensure
//! that mutable state is safely accessed.

pub mod text;
pub mod project;

use crate::prelude::*;

/// General-purpose `Result` supporting any `Error`-compatible failures.
pub type FallibleResult<T> = Result<T,failure::Error>;

/// Macro defines `StrongHandle` and `WeakHandle` newtypes for handles storing
/// the type given in the argument.
///
/// This allows treating handles as separate types and fitting them with impl
/// methods of their own. Such implementation may allow
/// hiding from user gritty details of borrows usage behind nice, easy API.
pub macro_rules! make_handles {
    ($data_type:ty) => {
        /// newtype wrapper over StrongHandle.
        #[derive(Shrinkwrap)]
        #[derive(Clone,Debug)]
        pub struct Handle(Rc<RefCell<$data_type>>);

        impl Handle {
            /// Obtain a WeakHandle to this data.
            pub fn downgrade(&self) -> WeakHandle {
                WeakHandle(self.0.downgrade())
            }
            /// Create a new StrongHandle that will wrap given data.
            pub fn new(data:$data_type) -> StrongHandle {
                StrongHandle(utils::cell::StrongHandle::new(data))
            }
        }

        /// newtype wrapper over WeakHandle.
        #[derive(Shrinkwrap)]
        #[derive(Clone,Debug)]
        pub struct WeakHandle(Weak<RefCell<<$data_type>>);

        impl WeakHandle {
            /// Obtain a StrongHandle to this data.
            pub fn upgrade(&self) -> Option<Handle> {
                self.0.upgrade().map(StrongHandle)
            }
        }
    };
}



// =========================
// === Module controller ===
// =========================

/// Module controller.
pub mod module {
    use super::*;

    /// Structure uniquely identifying module location in the project.
    /// Mappable to filesystem path.
    #[derive(Clone,Debug,Eq,Hash,PartialEq)]
    pub struct Location(pub String);
    impl Location {
        /// Obtains path (within a project context) to the file with this module.
        pub fn to_path(&self) -> file_manager_client::Path {
            // TODO [mwu] Extremely provisional. When multiple files support is
            //            added, needs to be fixed, if not earlier.
            let result = format!("./{}.luna", self.0);
            file_manager_client::Path::new(result)
        }
    }

    /// State data of the module controller.
    #[derive(Clone,Debug)]
    pub struct Data {
        /// This module's location.
        pub loc      : Location,
        /// Contents of the module file.
        pub contents : String,
        /// Handle to the project.
        pub parent   : project::StrongHandle,
    }

    impl Data {
        /// Fetches the Luna code for this module using remote File Manager.
        pub fn fetch_text(&self) -> impl Future<Output = FallibleResult<String>> {
            let loc    = self.loc.clone();
            let parent = self.parent.clone();
            // TODO [mwu] When metadata support is added, they will need to be
            //            stripped together with idmap from the source code.
            async move {
                parent.read_module(loc).await
            }
        }
    }

    make_handles!(Data);

    impl StrongHandle {
        /// Fetches the Luna code for this module using remote File Manager.
        pub fn fetch_text(&self) -> impl Future<Output = FallibleResult<String>> {
            self.with(|data| data.fetch_text()).flatten()
        }

        /// Receives a notification call when file with this module has been
        /// modified by a third-party tool (like non-IDE text editor).
        pub async fn file_externally_modified(&self) {
            // TODO: notify underlying text/graph controllers about the changes
            todo!()
        }
    }
}
