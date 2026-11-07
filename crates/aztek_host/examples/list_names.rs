//! Prints every command name, one per line, for checking coverage.
fn main() {
    let c = aztek_host::Core::new();
    let mut v: Vec<String> = c.s.commands.all().map(|d| d.name.clone()).collect();
    v.extend(aztek_host::mel::ALIASES.iter().map(|a| a.the_reference.to_string()));
    for n in v {
        println!("{n}");
    }
}
