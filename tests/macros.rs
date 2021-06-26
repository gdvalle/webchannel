#[macro_export]
macro_rules! server_test {
    ($name:ident, $config_file:tt, $fun:expr) => {
        #[test]
        fn $name() {
            let (mut handle, addr) = crate::util::start_server($config_file);
            let result = std::panic::catch_unwind(|| {
                $fun(addr);
            });
            handle.kill().unwrap();
            result.unwrap();
        }
    };
}

#[macro_export]
macro_rules! aw {
    ($e:expr) => {
        tokio_test::block_on($e)
    };
}
