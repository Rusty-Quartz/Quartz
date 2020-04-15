// Parses the mappings.txt, parser.rs_data, and protocol.json into a rust source file to parse all the packets
import { readFileSync, writeFileSync } from 'fs';

// Info about the parser
let quartzDir = '../';
let oreDir = './';
let version = 'v0.0.1';

// Handle command line args
let args = process.argv;
args.shift();
args.shift();

if(args.includes('-v') || args.includes('--version')) {
	console.log(`Pickaxe ${version}`);
	process.exit();
}
if(args.includes('-c') || args.includes('--configDir') || args.includes('--oreDir')) {
	let i = args.indexOf('-c') + 1 || args.indexOf('--configDir') + 1 || args.indexOf('--oreDir') + 1;
	oreDir = args[i];
	if(!oreDir.endsWith('/')) oreDir += '/';
}
if(args.includes('-o') || args.includes('--outputDir') || args.includes('--quartzDir')) {
	let i = args.indexOf('-o') + 1 || args.indexOf('--outputDir') + 1 || args.indexOf('--quartzDir') + 1;
	quartzDir = args[i];
	if(!quartzDir.endsWith('/')) quartzDir += '/';
}

// Read in all packet info
console.log('Loading protocol.json...');
let packetInfo: State[] = require(`${oreDir}protocol.json`);


// Read in all type mappings
console.log('Loading in mappings.json...');
let mappingsRaw:Mappings = require(`${oreDir}mappings.json`);

let mappings = new Map<string, string>();

mappingsRaw.types.forEach((mapping) => mappings.set(mapping.name, mapping.type));

let parseType = (type:string) => mappings.has(type.split('(')[0]) ? mappings.get(type.split('(')[0]) : type.split('(')[0];


// Read all the states and packets

let states: string[] = [];
let server_bound: Packet[] = [];
let client_bound: Packet[] = [];


console.log('Extracting client bound and server bound packet data...');
packetInfo.forEach((state) => {
	states.push(state.name);

	if(state.client_bound) state.client_bound.forEach(packet => {
		client_bound.push(packet);
	});

	if(state.server_bound) state.server_bound.forEach(packet => {
		server_bound.push(packet);
	})
});


// Parse packets into enum

let packetEnumParser = (packetArr: Packet[]):string => {
	let packetEnum = '';
	packetArr.forEach((packet, i) => {
		// If there are no fields just output an element of the enum with the name of the packet
		if(packet.fields.length == 0) {
			packetEnum += `\t${packet.name.replace(/_/g, '')}${(i == packetArr.length - 1 ? '' : ',')}\n\n`;
			return
		}
		
		let packetString = '';
		
		packetString += `\t${packet.name.replace(/_/g, '')} {`;
		
		packet.fields.forEach((field, i) => {
			// format the fields into rust struct elements
			packetString += `\n\t\t${field.name}: ${parseType(field.type)}${i == packet.fields.length-1 ? '' : ', '}`;
		});
		
		packetString += `\n\t}${(i == packetArr.length - 1 ? '' : ',')}\n`;
		
		// Add the element to the enum
		packetEnum += packetString;
	});
	return packetEnum
};

console.log('Parsing packets into enum...');
let serverPacketEnum = packetEnumParser(server_bound);
let clientPacketEnum = packetEnumParser(client_bound);

// Split packets up into async and sync

console.log('Extracting sync and async...');
let syncPackets: Packet[] = server_bound.filter((p) => !p.async);
let asyncPackets: Packet[] = server_bound.filter((p) => p.async);


// Parse async packets into AsyncPacketHandler

console.log('Parsing async packet data into AsyncPacketHandler...');
let asyncPacketHandler = '';

asyncPackets.forEach((packet) => {
	let asyncPacket = '';

	// Function definition header
	asyncPacket += `\tfn ${packet.name.toLowerCase()}(&mut self, client_conn: &mut AsyncClientConnection`;

	// function parameter
	packet.fields.forEach((field) => {
		asyncPacket += `, ${field.name}: ${parseType(field.type)}`
	});

	asyncPacket += ');\n';

	asyncPacketHandler += asyncPacket;
});

// Parse sync packets into SyncPacketHandler

console.log('Parsing sync packet data into SyncPacketHandler...');
let syncPacketHandler = '';

syncPackets.forEach((packet) => {
	let syncPacket = '';

	// define function for each sync packet
	syncPacket += `\tfn ${packet.name.toLowerCase()}(&mut self`;

	// have fields as parameters
	packet.fields.forEach((field) => {
		syncPacket += `, ${field.name}: ${parseType(field.type)}`
	});

	syncPacket += ');\n';

	syncPacketHandler += syncPacket;
});


// Write serializers and deserializers for each packet :)

console.log('Parsing server bound packet data into deserializers...');
let deserializers = '\tmatch conn.connection_state {';

// This code is hell
packetInfo.forEach((state, i) => {
	let stateString = `\n\t\tConnectionState::${state.name} => {`;

	if(state.server_bound) {
		stateString += '\n\t\t\tmatch id {';
	
		state.server_bound.forEach((packet, i) => {

			let packetString = `\n\t\t\t\t${packet.id} => {`;

			let varNames:string[] = [];

			// Loop over fields to make buffer.read function names
			packet.fields.forEach((field) => {
				packetString += `\n\t\t\t\t\tlet ${field.name} = buffer.read_${field.type + (field.type.includes('(') ? '' : '()')};`;
				varNames.push(field.name);
			});

			// determine if the packet is async or not
			if(packet.async) {
				// if async send packet data to the corrisponding async handler function
				packetString += `\n\t\t\t\t\tasync_handler.${packet.name.toLowerCase()}(conn, ${packet.fields.map(v => v.name).join(', ')});`;
				packetString += `\n\t\t\t\t}${i == (<Packet[]>state.server_bound).length - 1 ? '' : ','}`;
			} else {
				// otherwise yeet it to the server thread
				packetString += `\n\t\t\t\t\tconn.sync_packet_sender.send(ServerBoundPacket::${packet.name.replace(/_/g, '')}${packet.fields.length == 0 ? ');' : ' {'}`;

				// if no fields then just close the function without defining parameters for the struct
				if (packet.fields.length == 0) return stateString += `${packetString}\n\t\t\t\t}${i == packet.fields.length - 1 ? '' : ','}`;


				// put parameters for struct
				packet.fields.forEach((field, i) => {
					packetString += `${field.name}${i == packet.fields.length - 1 ? '' : ','}`;
				});

				packetString += `});\n\t\t\t\t}${i == (<Packet[]>state.server_bound).length - 1 ? '' : ','}`;
			}
			
			stateString += packetString;
			
		});

		stateString += `\n\t\t\t}`;
	}
	stateString += `\n\t\t}${i == packetInfo.length - 1 ? '' : ','}`;

	deserializers += stateString;
});

deserializers += '\n\t}'


console.log('Parsing client bound packet data into serializers...');
let serializers = '\tmatch packet {';

client_bound.forEach((packet, i) => {
	let packetString = `\n\t\tClientBoundPacket::${packet.name.replace(/_/g, '')}{${packet.fields.map((v) => v.name).join(', ')}} => {`;

	// Write length
	packetString += `\n\t\t\tbuffer.write_varint(${parseInt(packet.id, 16)});`;

	// Write each field to buffer
	packet.fields.forEach((fields) => {
		packetString += `\n\t\t\tbuffer.write_${fields.type}(${fields.type == 'string' || fields.type == 'byte_array' ? '&' : ''}${fields.name});`;
	});

	packetString += `\n\t\t}${i == client_bound.length - 1 ? '' : ','}`;

	serializers += packetString;
});

serializers += '\n\t}'

console.log('Parsing sync server bound packets into dispatchSyncPacket functions');
let dispatchSyncPacket = '\tmatch packet {';
syncPackets.forEach((packet, index) => {
	dispatchSyncPacket += `\n\t\tServerBoundPacket::${packet.name.replace(/_/g, '')}${packet.fields.length == 0 ? '' : `{${packet.fields.map(v => v.name).join(', ')}}`} => handler.${packet.name.toLowerCase()}(${packet.fields.map(v => v.name).join(', ')}),`;
});

dispatchSyncPacket += '\n\t\t_ => {}\n\t}';


let inclusiveIndexOf = (input: string, searchTerm: string) => {
	let index = input.indexOf(searchTerm);
	return index + (index == -1 ? 0 : searchTerm.length);
}


let outputFile = readFileSync(`${quartzDir}/src/network/packet_handler.rs`).toString();
// let outputFile = readFileSync(`${quartzDir}/parsed.rs`).toString();

// Get index of each marker
console.log('Geting location of the markers');
let asyncPackerHandlerIndex = inclusiveIndexOf(outputFile, '//#AsyncPacketHandler');
let syncPacketHandlerIndex = inclusiveIndexOf(outputFile, '//#SyncPacketHandler');
let clientPacketIndex = inclusiveIndexOf(outputFile,'//#ClientBoundPacket');
let serverPacketIndex = inclusiveIndexOf(outputFile, '//#ServerBoundPacket');
let dispatchSyncPacketIndex = inclusiveIndexOf(outputFile,'//#dispatch_sync_packet');
let serializeIndex = inclusiveIndexOf(outputFile,'//#serialize');
let handlePacketIndex = inclusiveIndexOf(outputFile,'//#handle_packet');

// put marker indexes and strings to insert into array
let insertionIndexes:[number, string][] = [[asyncPackerHandlerIndex, asyncPacketHandler], [syncPacketHandlerIndex, syncPacketHandler], [clientPacketIndex, clientPacketEnum], [serverPacketIndex, serverPacketEnum], [dispatchSyncPacketIndex, dispatchSyncPacket], [serializeIndex, serializers], [handlePacketIndex, deserializers]];
insertionIndexes = insertionIndexes.sort((a,b) => a[0] - b[0]);
let lastIndex = 0;
let output = '';

console.log('Inserting generated code');
insertionIndexes.forEach((data) => {
	let endIndex = outputFile.indexOf('//#end', data[0]);

	let slice = outputFile.substring(lastIndex, data[0]);
	output += slice;

	output += '\n' + data[1] + (data[1].endsWith('\n') ? '' : '\n');

	lastIndex = endIndex;
});

output += outputFile.substring(lastIndex, outputFile.length);


console.log('Writing code to file');
writeFileSync(`${quartzDir}/src/network/packet_handler.rs`, output);
console.log('Done!');

// Type declarations
type State = {
	name: string
	server_bound?: Packet[],
	client_bound?: Packet[]
};

type Packet = {
	async: boolean,
	name: string,
	id: string,
	fields: Field[]
}

type Field = {
	name: string,
	type: string
}

type Mappings = {
	types: TypeMap[]
}

type TypeMap = {
	name: string,
	type: string
}