class Keyboard {
  static init(): void {}
  static keyPressed(): number {
    return Memory.peek(24576);
  }
  // TODO: implement these
  static readChar(): void {}
  static readLine(): void {}
  static readInt(): void {}
}
