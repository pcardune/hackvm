class Sys {
  static init(): void {
    Memory.init();
    Math.init();
    Screen.init();
    Output.init();
    Keyboard.init();
    Main.main();
    Sys.halt();
  }

  static halt(): void {}
  static wait(): void {}
  static error(): void {}
}
