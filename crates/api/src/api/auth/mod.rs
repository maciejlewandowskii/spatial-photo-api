pub mod login;
pub mod register;
pub mod refresh;
pub mod logout;
pub mod me;
pub mod keys;
pub mod utils;

pub use register::handler as register_handler;
pub use login::handler as login_handler;
pub use refresh::handler as refresh_handler;
pub use logout::handler as logout_handler;
pub use logout::logout_get;
pub use me::handler as me_handler;
pub use keys::create as create_key;
pub use keys::list as list_keys;
pub use keys::delete as delete_key;

// Re-export the okapi functions for the routes! macro in api.rs
pub use register::okapi_add_operation_for_handler_ as okapi_add_operation_for_register_handler_;
pub use login::okapi_add_operation_for_handler_ as okapi_add_operation_for_login_handler_;
pub use refresh::okapi_add_operation_for_handler_ as okapi_add_operation_for_refresh_handler_;
pub use logout::okapi_add_operation_for_handler_ as okapi_add_operation_for_logout_handler_;
pub use logout::okapi_add_operation_for_logout_get_ as okapi_add_operation_for_logout_get_;
pub use me::okapi_add_operation_for_handler_ as okapi_add_operation_for_me_handler_;
pub use keys::okapi_add_operation_for_create_ as okapi_add_operation_for_create_key_;
pub use keys::okapi_add_operation_for_list_ as okapi_add_operation_for_list_keys_;
pub use keys::okapi_add_operation_for_delete_ as okapi_add_operation_for_delete_key_;
