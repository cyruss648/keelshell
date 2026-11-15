use keelshell_platform::Directory;
use std::fs::File;
use std::io;

pub fn inject(file: File) -> io::Result<Directory> {
    Directory::from_file(file)
}
