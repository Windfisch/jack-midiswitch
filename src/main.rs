use jack;
use clap;

fn main() {
	let args = clap::App::new("jack-midiswitch")
		.version("0.1.0")
		.author("Florian Jung <flo@windfis.ch>")
		.about("A keyboard controlled jack midi port multiplexer")
		.arg(clap::Arg::with_name("input")
			.short("i")
			.long("input")
			.takes_value(true)
			.multiple(true)
			.help("Input key mapping(s). Can be one or multiple strings, e.g. 'qwerty' and 'asdf'. This would create one input port that can be forwarded to six different output ports (selectable by the qwerty keys), and another input port that can be forwarded to four output ports.")
		)
		.arg(clap::Arg::with_name("output")
			.short("o")
			.long("output")
			.takes_value(true)
			.multiple(true)
			.help("Output key mapping(s). Same syntax as above, but creates N input ports that are mapped to one output ports.")
		)
		.get_matches();

	let input_mappings = args.values_of_lossy("input").unwrap_or(vec![]);
	let output_mappings = args.values_of_lossy("output").unwrap_or(vec![]);

	println!("{:?}", input_mappings);

    println!("Hello, world!");
}
