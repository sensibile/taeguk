pub fn f() {
    println!("{}", crate::cli::widgets::draw as usize);
}

macro_rules! again {
    () => {
        $crate::cli::run()
    };
}
