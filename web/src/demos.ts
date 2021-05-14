export const OSFiles = [
  "programs/OS/Array.vm",
  "programs/OS/Keyboard.vm",
  "programs/OS/Math.vm",
  "programs/OS/Memory.vm",
  "programs/OS/Output.vm",
  "programs/OS/Screen.vm",
  "programs/OS/String.vm",
  "programs/OS/Sys.vm",
];

type Demo = {
  title: string;
  files: string[];
  projectUrl: string;
  author: string;
  ticksPerCycle?: number;
  instructions?: string;
};

const demos: Record<string, Demo> = {
  pong: {
    title: "Pong",
    author: "Noam Nisan and Shimon Schocken (creators of Nand2Tetris)",
    projectUrl: "https://www.nand2tetris.org/",
    instructions: "Use the arrow keys to move the paddle.",
    files: [
      "programs/Pong/Bat.vm",
      "programs/Pong/Ball.vm",
      "programs/Pong/Main.vm",
      "programs/Pong/PongGame.vm",
    ],
  },
  hackenstein3D: {
    title: "Hackenstein 3D",
    author: "James Leibert",
    projectUrl: "https://github.com/QuesterZen/hackenstein3D",
    files: [
      "https://raw.githubusercontent.com/QuesterZen/hackenstein3D/master/dist/Display.vm",
      "https://raw.githubusercontent.com/QuesterZen/hackenstein3D/master/dist/Main.vm",
      "https://raw.githubusercontent.com/QuesterZen/hackenstein3D/master/dist/Player.vm",
      "https://raw.githubusercontent.com/QuesterZen/hackenstein3D/master/dist/Walls.vm",
    ],
  },
  scroller: {
    title: "Scroller",
    author: "Gavin Stewart",
    projectUrl: "https://github.com/gav-/Nand2Tetris-Games_and_Demos",
    files: [
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASscroller/GASscroller.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASscroller/Main.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASscroller/MathsToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASscroller/MemoryToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASscroller/Sinus.vm",
    ],
  },
  chunky: {
    title: "Chunky",
    author: "Gavin Stewart",
    projectUrl: "https://github.com/gav-/Nand2Tetris-Games_and_Demos",
    files: [
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/ChunkyImage.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Dither4x4.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/GASchunky.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Head.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Image.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Main.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/MathsToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/MemoryToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Monitor.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Plasma.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/RotoZoom.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASchunky/Sinus.vm",
    ],
  },
  boing: {
    title: "Boing",
    author: "Gavin Stewart",
    projectUrl: "https://github.com/gav-/Nand2Tetris-Games_and_Demos",
    files: [
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/GASboing.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/Image.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/Main.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/MathsToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/MemoryToo.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/Sinus.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball01.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball02.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball03.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball04.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball05.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball06.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball07.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/ball08.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow01.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow02.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow03.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow04.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow05.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow06.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow07.vm",
      "https://raw.githubusercontent.com/gav-/Nand2Tetris-Games_and_Demos/master/GASboing/shadow08.vm",
    ],
  },
};

export default demos;
