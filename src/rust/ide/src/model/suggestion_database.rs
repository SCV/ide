//! The module contains all structures for representing suggestions and their database.

use crate::prelude::*;

use crate::double_representation::module::QualifiedName;

use enso_protocol::language_server;
use language_server::types::SuggestionsDatabaseVersion;
use language_server::types::SuggestionDatabaseUpdateEvent;

pub use language_server::types::SuggestionEntryArgument as Argument;
pub use language_server::types::SuggestionEntryId as EntryId;
pub use language_server::types::SuggestionsDatabaseUpdate as Update;


// =============
// === Entry ===
// =============

/// A type of suggestion entry.
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
#[allow(missing_docs)]
pub enum EntryKind {
    Atom,Function,Local,Method
}

/// The Suggestion Database Entry.
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Entry {
    /// A name of suggested object.
    pub name : String,
    /// A type of suggestion.
    pub kind : EntryKind,
    /// A module where the suggested object is defined, represented as vector of segments.
    pub module : QualifiedName,
    /// Argument lists of suggested object (atom or function). If the object does not take any
    /// arguments, the list is empty.
    pub arguments : Vec<Argument>,
    /// A type returned by the suggested object.
    pub return_type : String,
    /// A documentation associated with object.
    pub documentation : Option<String>,
    /// A type of the "self" argument. This field is `None` for non-method suggestions.
    pub self_type : Option<String>,
}

impl Entry {
    /// Create entry from the structure deserialized from the Language Server responses.
    pub fn from_ls_entry(entry:language_server::types::SuggestionEntry) -> FallibleResult<Self> {
        use language_server::types::SuggestionEntry::*;
        let this = match entry {
            SuggestionEntryAtom {name,module,arguments,return_type,documentation} =>
                Self {
                    name,arguments,return_type,documentation,
                    module    : module.try_into()?,
                    self_type : None,
                    kind      : EntryKind::Atom,
                },
            SuggestionEntryMethod {name,module,arguments,self_type,return_type,documentation} =>
                Self {
                    name,arguments,return_type,documentation,
                    module    : module.try_into()?,
                    self_type : Some(self_type),
                    kind      : EntryKind::Method,
                },
            SuggestionEntryFunction {name,module,arguments,return_type,..} =>
                Self {
                    name,arguments,return_type,
                    module        : module.try_into()?,
                    self_type     : None,
                    documentation : default(),
                    kind          : EntryKind::Function,
                },
            SuggestionEntryLocal {name,module,return_type,..} =>
                Self {
                    name,return_type,
                    arguments     : default(),
                    module        : module.try_into()?,
                    self_type     : None,
                    documentation : default(),
                    kind          : EntryKind::Local,
                },
        };
        Ok(this)
    }

    /// Returns the code which should be inserted to Searcher input when suggestion is picked.
    pub fn code_to_insert(&self) -> String {
        let module = self.module.name();
        if self.self_type.as_ref().contains(&module) {
            iformat!("{module}.{self.name}")
        } else {
            self.name.clone()
        }
    }

    /// Returns entry with the changed name.
    pub fn with_name(self, name:impl Into<String>) -> Self {
        Self {name:name.into(),..self}
    }
}

impl TryFrom<language_server::types::SuggestionEntry> for Entry {
    type Error = failure::Error;
    fn try_from(entry:language_server::types::SuggestionEntry) -> FallibleResult<Self> {
        Self::from_ls_entry(entry)
    }
}



// ================
// === Database ===
// ================

/// The Suggestion Database
///
/// This is database of possible suggestions in Searcher. To achieve best performance, some
/// often-called Language Server methods returns the list of keys of this database instead of the
/// whole entries. Additionally the suggestions contains information about functions and their
/// argument names and types.
#[derive(Clone,Debug,Default)]
pub struct SuggestionDatabase {
    logger  : Logger,
    entries : RefCell<HashMap<EntryId,Rc<Entry>>>,
    version : Cell<SuggestionsDatabaseVersion>,
}

impl SuggestionDatabase {
    /// Create a new database which will take its initial content from the Language Server.
    pub async fn create_synchronized
    (language_server:&language_server::Connection) -> FallibleResult<Self> {
        let response = language_server.client.get_suggestions_database().await?;
        Ok(Self::from_ls_response(response))
    }

    /// Create a new database model from response received from the Language Server.
    fn from_ls_response(response:language_server::response::GetSuggestionDatabase) -> Self {
        let logger      = Logger::new("SuggestionDatabase");
        let mut entries = HashMap::new();
        for ls_entry in response.entries {
            let id = ls_entry.id;
            match Entry::from_ls_entry(ls_entry.suggestion) {
                Ok(entry) => { entries.insert(id, Rc::new(entry)); },
                Err(err)  => { error!(logger,"Discarded invalid entry {id}: {err}"); },
            }
        }
        Self {
            logger,
            entries : RefCell::new(entries),
            version : Cell::new(response.current_version),
        }
    }

    /// Get suggestion entry by id.
    pub fn get(&self, id:EntryId) -> Option<Rc<Entry>> {
        self.entries.borrow().get(&id).cloned()
    }

    /// Apply the update event to the database.
    pub fn apply_update_event(&self, event:SuggestionDatabaseUpdateEvent) {
        for update in event.updates {
            let mut entries = self.entries.borrow_mut();
            match update {
                Update::Add {id,entry} => match entry.try_into() {
                    Ok(entry) => { entries.insert(id,Rc::new(entry));                       },
                    Err(err)  => { error!(self.logger, "Discarding update for {id}: {err}") },
                },
                Update::Remove {id} => { entries.remove(&id); },
            };
        }
        self.version.set(event.current_version);
    }

    /// Put the entry to the database. Using this function likely break the synchronization between
    /// Language Server and IDE, and should be used only in tests.
    #[cfg(test)]
    pub fn put_entry(&self, id:EntryId, entry:Entry) {
        self.entries.borrow_mut().insert(id,Rc::new(entry));
    }
}

impl From<language_server::response::GetSuggestionDatabase> for SuggestionDatabase {
    fn from(database:language_server::response::GetSuggestionDatabase) -> Self {
        Self::from_ls_response(database)
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
mod test {
    use super::*;

    use enso_protocol::language_server::SuggestionsDatabaseEntry;



    #[test]
    fn code_from_entry() {
        let module = QualifiedName::from_segments("Project",&["Main"]).unwrap();
        let atom_entry = Entry {
            name          : "Atom".to_string(),
            kind          : EntryKind::Atom,
            module,
            arguments     : vec![],
            return_type   : "Number".to_string(),
            documentation : None,
            self_type     : None
        };
        let method_entry = Entry {
            name      : "method".to_string(),
            kind      : EntryKind::Method,
            self_type : Some("Number".to_string()),
            ..atom_entry.clone()
        };
        let module_method_entry = Entry {
            name      : "moduleMethod".to_string(),
            self_type : Some("Main".to_string()),
            ..method_entry.clone()
        };

        assert_eq!(atom_entry.code_to_insert()         , "Atom".to_string());
        assert_eq!(method_entry.code_to_insert()       , "method".to_string());
        assert_eq!(module_method_entry.code_to_insert(), "Main.moduleMethod".to_string());
    }

    #[test]
    fn initialize_database() {
        // Empty db
        let response = language_server::response::GetSuggestionDatabase {
            entries         : vec![],
            current_version : 123
        };
        let db = SuggestionDatabase::from_ls_response(response);
        assert!(db.entries.borrow().is_empty());
        assert_eq!(db.version.get()    , 123);

        // Non-empty db
        let entry = language_server::types::SuggestionEntry::SuggestionEntryAtom {
            name          : "TextAtom".to_string(),
            module        : "TestProject.TestModule".to_string(),
            arguments     : vec![],
            return_type   : "TestAtom".to_string(),
            documentation : None
        };
        let db_entry = SuggestionsDatabaseEntry {id:12, suggestion:entry};
        let response = language_server::response::GetSuggestionDatabase {
            entries         : vec![db_entry],
            current_version : 456
        };
        let db = SuggestionDatabase::from_ls_response(response);
        assert_eq!(db.entries.borrow().len(), 1);
        assert_eq!(*db.get(12).unwrap().name, "TextAtom".to_string());
        assert_eq!(db.version.get(), 456);
    }

    #[test]
    fn applying_update() {
        let entry1 = language_server::types::SuggestionEntry::SuggestionEntryAtom {
            name          : "Entry1".to_string(),
            module        : "TestProject.TestModule".to_string(),
            arguments     : vec![],
            return_type   : "TestAtom".to_string(),
            documentation : None
        };
        let entry2 = language_server::types::SuggestionEntry::SuggestionEntryAtom {
            name          : "Entry2".to_string(),
            module        : "TestProject.TestModule".to_string(),
            arguments     : vec![],
            return_type   : "TestAtom".to_string(),
            documentation : None
        };
        let new_entry2 = language_server::types::SuggestionEntry::SuggestionEntryAtom {
            name          : "NewEntry2".to_string(),
            module        : "TestProject.TestModule".to_string(),
            arguments     : vec![],
            return_type   : "TestAtom".to_string(),
            documentation : None
        };

        let db_entry1        = SuggestionsDatabaseEntry {id:1, suggestion:entry1};
        let db_entry2        = SuggestionsDatabaseEntry {id:2, suggestion:entry2};
        let initial_response = language_server::response::GetSuggestionDatabase {
            entries         : vec![db_entry1,db_entry2],
            current_version : 1,
        };
        let db = SuggestionDatabase::from_ls_response(initial_response);

        // Remove
        let remove_update = Update::Remove {id:2};
        let update        = SuggestionDatabaseUpdateEvent {
            updates         : vec![remove_update],
            current_version : 2
        };
        db.apply_update_event(update);
        assert_eq!(db.get(2),        None);
        assert_eq!(db.version.get(), 2   );

        // Add
        let add_update = Update::Add {id:2, entry:new_entry2};
        let update     = SuggestionDatabaseUpdateEvent {
            updates         : vec![add_update],
            current_version : 3,
        };
        db.apply_update_event(update);
        assert_eq!(db.get(2).unwrap().name, "NewEntry2");
        assert_eq!(db.version.get(),        3          );
    }
}
