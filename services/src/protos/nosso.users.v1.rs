#[derive(Clone, PartialEq, ::prost::Message)]
pub struct User {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub email: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub password: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
}
