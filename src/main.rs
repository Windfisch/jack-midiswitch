use jack;
use clap;
use termion;
use ringbuf;
use termion::raw::IntoRawMode;
use termion::event::Key;
use termion::input::TermRead;
use std::io::{Write, stdin, stdout};

enum Message {
	Merge(usize, usize),
	Split(usize, usize)
}

fn main() {
	let args = clap::App::new("jack-midiswitch")
		.version("0.1.0")
		.author("Florian Jung <flo@windfis.ch>")
		.about("A keyboard controlled jack midi port multiplexer")
		.arg(clap::Arg::with_name("split")
			.short("s")
			.long("split")
			.takes_value(true)
			.multiple(true)
			.help("Input key mapping(s). Can be one or multiple strings, e.g. 'qwerty' and 'asdf'. This would create one input port that can be forwarded to six different output ports (selectable by the qwerty keys), and another input port that can be forwarded to four output ports.")
		)
		.arg(clap::Arg::with_name("merge")
			.short("m")
			.long("merge")
			.takes_value(true)
			.multiple(true)
			.help("Output key mapping(s). Same syntax as above, but creates N input ports that are mapped to one output ports.")
		)
		.arg(clap::Arg::with_name("client_name")
			.short("n")
			.long("client_name")
			.help("The jack client name (default: 'midiswitch')")
		)
		.get_matches();

	let split_mappings: Vec<String> = args.values_of_lossy("split").unwrap_or(vec![]).into_iter().filter(|s| !s.is_empty()).collect();
	let merge_mappings: Vec<String> = args.values_of_lossy("merge").unwrap_or(vec![]).into_iter().filter(|s| !s.is_empty()).collect();
	let client_name: String = args.value_of_lossy("client_name").map(|s| s.into()).unwrap_or("midiswitch".into());

	let client = jack::Client::new(&client_name, jack::ClientOptions::NO_START_SERVER).expect("Failed to connect to JACK").0;
	
	let mut split_ports: Vec<_> = split_mappings.iter().map(|mapping| {
		(
			0 as usize,
			client.register_port(&format!("{}_in", mapping), jack::MidiIn).expect(&format!("Failed to register input port {}_in", mapping)),
			mapping.chars().map(|character| {
				client.register_port(&format!("{}_out_{}", mapping, character), jack::MidiOut).expect(&format!("Failed to register output port {}_out_{}", mapping, character))
			}).collect::<Vec<_>>()
		)
	}).collect();

	let mut merge_ports: Vec<_> = merge_mappings.iter().map(|mapping| {
		(
			0 as usize,
			client.register_port(&format!("{}_out", mapping), jack::MidiOut).expect(&format!("Failed to register output port {}_out", mapping)),
			mapping.chars().map(|character| {
				client.register_port(&format!("{}_in_{}", mapping, character), jack::MidiIn).expect(&format!("Failed to register input port {}_in_{}", mapping, character))
			}).collect::<Vec<_>>(),
			false
		)
	}).collect();

	let mut selections: Vec<_> = split_mappings.iter().chain(merge_mappings.iter()).map(|mapping| {
		(mapping, 0 as usize)
	}).collect();

	let (mut producer, mut consumer) = ringbuf::RingBuffer::<Message>::new(10).split();

	let _async_client = client.activate_async((), jack::ClosureProcessHandler::new(move |_client: &jack::Client, scope: &jack::ProcessScope| -> jack::Control {
		// hack to clear all midi buffers
		for (_,_,out_ports) in split_ports.iter_mut() {
			for out_port in out_ports.iter_mut() {
				out_port.writer(scope);
			}
		}

		// apply updates from the ringbuffer
		while let Some(message) = consumer.pop() {
			match message {
				Message::Merge(index, port) => {
					assert!(port < merge_ports[index].2.len());
					merge_ports[index].0 = port;
					merge_ports[index].3 = true;
				}
				Message::Split(index, port) => {
					let old_index = split_ports[index].0;
					all_notes_off(&mut split_ports[index].2[old_index].writer(scope));
					assert!(port < split_ports[index].2.len());
					split_ports[index].0 = port;
				}
			}
		}

		// copy events from activated in->out port connections
		for (out_idx, in_port, out_ports) in split_ports.iter_mut() {
			let mut writer = out_ports[*out_idx].writer(scope);
			for event in in_port.iter(scope) {
				writer.write(&event).ok();
			}
		}
		for (in_idx, out_port, in_ports, all_notes_off_pending) in merge_ports.iter_mut() {
			let mut writer = out_port.writer(scope);

			if *all_notes_off_pending {
				all_notes_off(&mut writer);
				*all_notes_off_pending = false;
			}

			for event in in_ports[*in_idx].iter(scope) {
				writer.write(&event).ok();
			}
		}

		return jack::Control::Continue;
	})).expect("Failed to activate client");

	let stdin = stdin();
	let mut stdout = stdout().into_raw_mode().expect("Failed to enable terminal raw mode");

	print!("ctrl-c to exit.\r\n");
	stdout.flush().unwrap();
	
	display(&selections);

	for input in stdin.keys() {
		// update the selection
		match input.unwrap() {
			Key::Ctrl('c') => { break; }
			Key::Char(chr) => {
				for (index, (mapping, selected)) in selections.iter_mut().enumerate() {
					if let Some(pos) = mapping.find(chr) {
						*selected = pos;

						if index < split_mappings.len() {
							producer.push(Message::Split(index, pos)).map_err(|_|()).expect("Failed to push to the queue");
						}
						else {
							producer.push(Message::Merge(index - split_mappings.len(), pos)).map_err(|_|()).expect("Failed to push to the queue");
						}
					}
				}
			}
			_ => {}
		}

		display(&selections);
	}

	print!("\r\n");
}

fn all_notes_off(writer: &mut jack::MidiWriter)
{
	for channel in 0..16 {
		writer.write(&jack::RawMidi {
			time: 0,
			bytes: &[0xB0 | channel as u8, 11, 42]
		}).ok();
	}
}

fn display(selections: &Vec<(&String, usize)>) {
	print!("{}\r", termion::clear::CurrentLine);
	for (mapping, index) in selections.iter() {
		for (i, chr) in mapping.chars().enumerate() {
			if i == *index {
				print!("[{}] ", chr);
			}
			else {
				print!(" {}  ", chr);
			}
		}
		print!(" |  ");
	}
	stdout().flush().unwrap();
}
