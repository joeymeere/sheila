use sheila_proc_macros as sheila;

#[sheila::suite]
pub mod math_tests {
    use super::Calculator;

    #[sheila::fixture(scope = "suite")]
    fn number_generator() -> u64 {
        let num = rand::random::<u64>();
        num
    }

    #[sheila::fixture(scope = "suite", depends_on = ["number_generator"])]
    pub fn calculator() -> Calculator {
        Calculator::new()
    }

    #[sheila::fixture(scope = "test", depends_on = ["number_generator", "calculator"])]
    pub fn log_calculations() {
        println!("Logging calculations");
    }

    #[sheila::before_all]
    pub fn setup_math_environment() {
        println!("Setting up math test environment");
    }

    #[sheila::test(tags = ["basic_ops"])]
    pub fn test_addition() {
        let calc = calculator();
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert_eq!(calc.add(2, 2), 4);
    }

    #[sheila::test(tags = ["basic_ops"])]
    pub fn test_subtraction() {
        let calc = calculator();
        std::thread::sleep(std::time::Duration::from_secs(10));
        assert_eq!(calc.subtract(5, 3), 2);
    }

    #[sheila::test]
    pub fn test_division() {
        let calc = calculator();
        std::thread::sleep(std::time::Duration::from_secs(7));
        assert_eq!(calc.divide(10, 2), 5);
    }
}

#[derive(Debug)]
pub struct Calculator;

impl Calculator {
    pub fn new() -> Self {
        Self
    }

    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    pub fn divide(&self, a: i32, b: i32) -> i32 {
        a / b
    }
}
