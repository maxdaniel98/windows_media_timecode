# Windows Media Timecode
> Windows Media Controls as MIDI MTC Timecode!


## Description 
This software monitors the Windows Media Controls for song playback, and outputs the current position of a song as MIDI MTC Timecode. It is also possible to set offsets for specific songs, or disable outputting of MIDI for songs that are not in the config. 

## Download
Download the latest version from the [Releases page](https://github.com/maxdaniel98/windows_media_timecode/releases/latest). 

## Config
You should provide a `config.json` file in the working directory of the program, or start the software with an config file as argument, for example `windows_media_timecode.exe config-1.json`. 

This is a config example:

```{
  "midiDevice": "loopmidi",
  "disableSongsOutsideConfig": true,
  "songs": [
    {
      "artist": "Sam Smith",
      "title": "Unholy (feat. Kim Petras) - David Guetta Acid Remix",
      "timecodeOffset": 1200000
    },
	{
      "artist": "Gustaph",
      "title": "Because Of You",
      "timecodeOffset": 1800000
    },
	{
      "artist": "WALK THE MOON",
      "title": "Different Colors",
      "timecodeOffset": 0
    },
	{
      "artist": "Daði Freyr",
      "title": "Whole Again",
      "timecodeOffset": 600000
    }
  ]
}
```
- **midiDevice** : The name of the midi device you want to output MIDI on. When not provided, the software will ask which device you want to output on _Optional_
- **disableSongsOutsideConfig**: Disable output of MIDI Timecode of songs that are not in the songs array _Optional_
- **songs**: This array contains all the songs you would like to have MIDI for (when disableSongsOutsideConfig is enabled) or you want to change the offset of the MIDI Timecode for
   - artist: The song artist
   - title: The song title
   - timecodeOffset: The offset of the song in milliseconds. (In the example above, Because Of You starts at 30 minutes). 
