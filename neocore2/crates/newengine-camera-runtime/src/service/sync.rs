use super::*;

// Render-cadence camera synchronization is split by responsibility while retaining one private module.
include!("sync/first_person.rs");
include!("sync/third_person.rs");
include!("sync/service.rs");
include!("sync/first_person_tests.rs");
include!("sync/third_person_tests.rs");
