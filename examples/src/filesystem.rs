use sheila_proc_macros as sheila;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn temp_fs() -> FileSystem {
    let temp_dir = std::env::temp_dir().join(format!("sheila_test_{}", std::process::id()));
    FileSystem::new(temp_dir).expect("Failed to create temp filesystem")
}

#[sheila::suite]
pub mod filesystem_tests {
    use super::{FileSystem, temp_fs};
    use std::path::PathBuf;

    #[sheila::fixture]
    fn temp_filesystem() -> FileSystem {
        temp_fs()
    }

    #[sheila::fixture]
    fn sample_text_content() -> String {
        "Hello, World!\nThis is a test file.\nLine 3\nLine 4".to_string()
    }

    #[sheila::fixture]
    fn large_text_content() -> String {
        (0..1000)
            .map(|i| format!("Line {}: This is line number {}\n", i, i))
            .collect::<String>()
    }

    #[sheila::before_all]
    fn setup_filesystem_environment() {
        println!("- Setting up file system test environment...");
    }

    #[sheila::after_all]
    fn cleanup_filesystem_environment() {
        println!("- Cleaning up file system test environment...");
    }

    #[sheila::before_each]
    fn setup_test_files() {
        println!("  - Setting up test files...");
    }

    #[sheila::after_each]
    fn cleanup_test_files() {
        println!("  -  Cleaning up test files...");
    }

    #[sheila::test(tags = ["filesystem", "files", "basic"])]
    fn test_create_and_read_file() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("test.txt", &content)
            .expect("Failed to create file");

        assert!(fs.file_exists("test.txt"));

        let read_content = fs.read_file("test.txt").expect("Failed to read file");
        assert_eq!(read_content, content);

        println!("✓ File created and read successfully");

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    #[sheila::test(tags = ["filesystem", "files", "basic"])]
    fn test_append_to_file() {
        let fs = temp_filesystem();
        let initial_content = "Initial content\n";
        let append_content = "Appended content\n";

        fs.create_file("append_test.txt", initial_content)
            .expect("Failed to create file");

        fs.append_to_file("append_test.txt", append_content)
            .expect("Failed to append to file");

        let final_content = fs
            .read_file("append_test.txt")
            .expect("Failed to read file");
        assert_eq!(
            final_content,
            format!("{}{}", initial_content, append_content)
        );

        println!("✓ Content appended successfully");

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    #[sheila::test(tags = ["filesystem", "files", "operations"])]
    fn test_copy_file() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("source.txt", &content)
            .expect("Failed to create source file");

        fs.copy_file("source.txt", "copy.txt")
            .expect("Failed to copy file");

        assert!(fs.file_exists("source.txt"));
        assert!(fs.file_exists("copy.txt"));

        let source_content = fs.read_file("source.txt").expect("Failed to read source");
        let copy_content = fs.read_file("copy.txt").expect("Failed to read copy");
        assert_eq!(source_content, copy_content);

        println!("✓ File copied successfully");

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    #[sheila::test(tags = ["filesystem", "files", "operations"])]
    fn test_move_file() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("move_source.txt", &content)
            .expect("Failed to create source file");

        fs.move_file("move_source.txt", "moved.txt")
            .expect("Failed to move file");

        assert!(!fs.file_exists("move_source.txt"));
        assert!(fs.file_exists("moved.txt"));

        let moved_content = fs
            .read_file("moved.txt")
            .expect("Failed to read moved file");
        assert_eq!(moved_content, content);

        println!("✓ File moved successfully");

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    #[sheila::test(tags = ["filesystem", "files", "operations"])]
    fn test_delete_file() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("delete_me.txt", &content)
            .expect("Failed to create file");
        assert!(fs.file_exists("delete_me.txt"));

        fs.delete_file("delete_me.txt")
            .expect("Failed to delete file");

        assert!(!fs.file_exists("delete_me.txt"));

        println!("✓ File deleted successfully");

        let _ = fs.cleanup();
    }

    #[sheila::test(tags = ["filesystem", "directories", "basic"])]
    fn test_create_directory() {
        let fs = temp_filesystem();
        fs.create_directory("test_dir")
            .expect("Failed to create directory");

        assert!(fs.file_exists("test_dir"));

        let info = fs
            .get_file_info("test_dir")
            .expect("Failed to get directory info");
        assert!(info.is_dir);
        println!("✓ Directory created successfully");

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[sheila::test(tags = ["filesystem", "directories", "nested"])]
    fn test_create_nested_directories() {
        let fs = temp_filesystem();

        fs.create_directory("level1/level2/level3")
            .expect("Failed to create nested directories");

        assert!(fs.file_exists("level1"));
        assert!(fs.file_exists("level1/level2"));
        assert!(fs.file_exists("level1/level2/level3"));

        fs.create_file("level1/level2/level3/nested_file.txt", "nested content")
            .expect("Failed to create file in nested directory");

        assert!(fs.file_exists("level1/level2/level3/nested_file.txt"));
        println!("✓ Nested directories created successfully");

        let _ = fs.cleanup();
    }

    #[sheila::test(tags = ["filesystem", "directories", "operations"])]
    fn test_list_files_in_directory() {
        let fs = temp_filesystem();

        fs.create_directory("list_test")
            .expect("Failed to create directory");
        fs.create_file("list_test/file1.txt", "content1")
            .expect("Failed to create file1");
        fs.create_file("list_test/file2.txt", "content2")
            .expect("Failed to create file2");
        fs.create_file("list_test/file3.txt", "content3")
            .expect("Failed to create file3");

        let files = fs.list_files("list_test").expect("Failed to list files");

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file1.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file2.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file3.txt"));

        println!("✓ Listed {} files in directory", files.len());

        let _ = fs.cleanup();
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    #[sheila::test(tags = ["filesystem", "directories", "operations"])]
    fn test_delete_directory() {
        let fs = temp_filesystem();

        fs.create_directory("delete_dir")
            .expect("Failed to create directory");
        fs.create_file("delete_dir/file1.txt", "content")
            .expect("Failed to create file");
        fs.create_file("delete_dir/subdir/file2.txt", "content")
            .expect("Failed to create nested file");

        assert!(fs.file_exists("delete_dir"));
        assert!(fs.file_exists("delete_dir/file1.txt"));

        fs.delete_directory("delete_dir")
            .expect("Failed to delete directory");

        assert!(!fs.file_exists("delete_dir"));
        println!("✓ Directory and contents deleted successfully");

        std::thread::sleep(std::time::Duration::from_secs(10));

        let _ = fs.cleanup();
    }

    #[sheila::test(tags = ["filesystem", "info", "metadata"])]
    fn test_get_file_info() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("info_test.txt", &content)
            .expect("Failed to create file");

        let info = fs
            .get_file_info("info_test.txt")
            .expect("Failed to get file info");

        assert!(!info.is_dir);
        assert_eq!(info.size, content.len() as u64);
        assert_eq!(info.path, PathBuf::from("info_test.txt"));

        println!(
            "✓ File info retrieved: {} bytes, permissions: {:o}",
            info.size, info.permissions
        );

        std::thread::sleep(std::time::Duration::from_secs(3));

        let _ = fs.cleanup();
    }

    #[sheila::test(tags = ["filesystem", "permissions"])]
    fn test_file_permissions() {
        let fs = temp_filesystem();
        let content = sample_text_content();

        fs.create_file("perm_test.txt", &content)
            .expect("Failed to create file");

        fs.set_permissions("perm_test.txt", 0o444)
            .expect("Failed to set permissions");

        let info = fs
            .get_file_info("perm_test.txt")
            .expect("Failed to get file info");
        assert_eq!(info.permissions & 0o777, 0o444);

        fs.set_permissions("perm_test.txt", 0o644)
            .expect("Failed to set permissions");

        let info = fs
            .get_file_info("perm_test.txt")
            .expect("Failed to get file info");
        assert_eq!(info.permissions & 0o777, 0o644);

        println!("✓ File permissions modified successfully");

        std::thread::sleep(std::time::Duration::from_secs(10));

        let _ = fs.cleanup();
    }

    #[sheila::test(timeout = 30, tags = ["filesystem", "performance", "large"], retries = 2)]
    fn test_large_file_operations() {
        let fs = temp_filesystem();
        let large_content = large_text_content();

        println!(
            "  📊 Testing large file operations ({} bytes)...",
            large_content.len()
        );

        let start = std::time::Instant::now();

        fs.create_file("large_file.txt", &large_content)
            .expect("Failed to create large file");
        println!("    ✓ Large file created in {:?}", start.elapsed());

        let read_start = std::time::Instant::now();

        let read_content = fs
            .read_file("large_file.txt")
            .expect("Failed to read large file");
        assert_eq!(read_content.len(), large_content.len());
        println!("    ✓ Large file read in {:?}", read_start.elapsed());

        let copy_start = std::time::Instant::now();
        fs.copy_file("large_file.txt", "large_file_copy.txt")
            .expect("Failed to copy large file");
        println!("    ✓ Large file copied in {:?}", copy_start.elapsed());

        let copy_content = fs
            .read_file("large_file_copy.txt")
            .expect("Failed to read copied file");
        assert_eq!(copy_content.len(), large_content.len());

        println!("✓ Large file operations completed in {:?}", start.elapsed());

        std::thread::sleep(std::time::Duration::from_secs(10));

        let _ = fs.cleanup();
    }

    #[sheila::test(tags = ["filesystem", "stress"], retries = 2)]
    fn test_many_small_files() {
        let fs = temp_filesystem();

        println!("  📊 Creating many small files...");

        for i in 0..100 {
            let filename = format!("small_file_{}.txt", i);
            let content = format!("Content for file {}", i);
            fs.create_file(&filename, &content)
                .expect("Failed to create small file");
        }

        let files = fs.list_files(".").expect("Failed to list files");
        assert!(files.len() >= 100);

        for i in 0..100 {
            let filename = format!("small_file_{}.txt", i);
            let content = fs.read_file(&filename).expect("Failed to read small file");
            assert_eq!(content, format!("Content for file {}", i));
        }

        println!("✓ Successfully created and read 100 small files");

        std::thread::sleep(std::time::Duration::from_secs(10));

        let _ = fs.cleanup();
    }
}

// Can also run cargo compatible tests standalone if not included in a suite
#[sheila::test(tags = ["filesystem", "errors"])]
fn test_file_not_found_errors() {
    let fs = temp_fs();

    let result = fs.read_file("nonexistent.txt");
    assert!(result.is_err());

    let result = fs.delete_file("nonexistent.txt");
    assert!(result.is_err());

    let result = fs.get_file_info("nonexistent.txt");
    assert!(result.is_err());

    println!("✓ File not found errors handled correctly");

    let _ = fs.cleanup();
}

#[derive(Debug)]
pub struct FileSystem {
    base_path: PathBuf,
}

#[derive(Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    pub permissions: u32,
}

impl FileSystem {
    pub fn new<P: AsRef<Path>>(base_path: P) -> std::io::Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self { base_path })
    }

    pub fn create_file<P: AsRef<Path>>(&self, path: P, content: &str) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(full_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(full_path)
    }

    pub fn append_to_file<P: AsRef<Path>>(&self, path: P, content: &str) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(full_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    pub fn create_directory<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        fs::create_dir_all(full_path)
    }

    pub fn delete_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        fs::remove_file(full_path)
    }

    pub fn delete_directory<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        fs::remove_dir_all(full_path)
    }

    pub fn file_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = self.base_path.join(path);
        full_path.exists()
    }

    pub fn get_file_info<P: AsRef<Path>>(&self, path: P) -> std::io::Result<FileInfo> {
        let full_path = self.base_path.join(path.as_ref());
        let metadata = fs::metadata(&full_path)?;

        Ok(FileInfo {
            path: path.as_ref().to_path_buf(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            permissions: metadata.permissions().mode(),
        })
    }

    pub fn list_files<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Vec<PathBuf>> {
        let full_path = self.base_path.join(path);
        let mut files = Vec::new();

        for entry in fs::read_dir(full_path)? {
            let entry = entry?;
            files.push(
                entry
                    .path()
                    .strip_prefix(&self.base_path)
                    .unwrap()
                    .to_path_buf(),
            );
        }

        files.sort();
        Ok(files)
    }

    pub fn copy_file<P: AsRef<Path>>(&self, from: P, to: P) -> std::io::Result<()> {
        let from_path = self.base_path.join(from);
        let to_path = self.base_path.join(to);

        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(from_path, to_path)?;
        Ok(())
    }

    pub fn move_file<P: AsRef<Path>>(&self, from: P, to: P) -> std::io::Result<()> {
        let from_path = self.base_path.join(from);
        let to_path = self.base_path.join(to);

        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::rename(from_path, to_path)?;
        Ok(())
    }

    pub fn set_permissions<P: AsRef<Path>>(&self, path: P, mode: u32) -> std::io::Result<()> {
        let full_path = self.base_path.join(path);
        let mut permissions = fs::metadata(&full_path)?.permissions();
        permissions.set_mode(mode);
        fs::set_permissions(full_path, permissions)
    }

    pub fn cleanup(&self) -> std::io::Result<()> {
        if self.base_path.exists() {
            fs::remove_dir_all(&self.base_path)?;
        }
        Ok(())
    }
}
