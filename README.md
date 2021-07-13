# Quartz
A reimplementation of a minecraft server in rust.  
Currently working on supporting 1.17

## Mission Statement

Be better than feather.

## Current Features
- [x] Logging
- [x] Console commands
- [x] Server List ping
- [x] Initial Join Game flow
- [x] Chunk loading

## Planned Features
We plan on trying to be as 1:1 to vanilla as we can with the exception of any current bugs in vanilla<br>
We also plan on having our own plugin system to allow extending the functionality of Quartz

## Related Repos
[Quartz Proxy](https://github.com/Rusty-Quartz/quartz_proxy), used to test reading and writing of packets and to log data the vanilla server sends<br>
[Quartz Commands](https://github.com/Rusty-Quartz/quartz_commands), the command system used to handle console and in-game commands<br>
[Quartz NBT](https://github.com/Rusty-Quartz/quartz_nbt), our nbt and snbt crate<br>
[Quartz Data Generators](https://github.com/Rusty-Quartz/data-generator), a mod for Minecraft that exports JSON files to be used in our buildscripts

### Credits
Packet info and minecraft datatypes from the [minecraft protocol wiki](https://wiki.vg/)  
Other info taken from minecraft source deobfuscated using [yarn mappings](https://github.com/FabricMC/yarn)
