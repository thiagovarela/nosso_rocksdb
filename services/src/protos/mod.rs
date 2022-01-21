pub mod google {
    pub mod protobuf {
        include!("google.protobuf.rs");
    }
}
pub mod nosso {
    pub mod users {
        pub mod v1 {
            include!("nosso.users.v1.rs");
        }
    }
}
