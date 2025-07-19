class Screen {
  static color: number;
  static init(): void {
    Screen.color = true;
  }

  static clearScreen(): void {
    let i: number = 0;
    let screen: number[] = 16384;
    while (i < 8192) {
      screen[i] = 0;
      i = i + 1;
    }
  }

  static setColor(color: number): void {
    Screen.color = color;
  }

  static drawRectangle(x1: number, y1: number, x2: number, y2: number): void {
    let screen: number[] = 16384;
    let bitmap: number[] = [
      1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384,
      32768,
    ];
    if (x1 > x2 || y1 > y2 || x1 < 0 || x2 > 511 || y1 < 0 || y2 > 255) {
      Sys.error(9);
    } else {
      while (y1 < y2) {
        let x: number = x1;
        while (x < x2) {
          let color: number = Screen.color;
          let lastBit: number = x - (x / 16) * 16;
          let i: number = 0;
          while (i < lastBit) {
            color = color | bitmap[i];
            i = i + 1;
          }
          screen[y1 * 32 + x / 16] = color;
          x = x + 16;
        }
        y1 = y1 + 1;
      }
    }
  }

  static drawLine(x1: number, y1: number, x2: number, y2: number): void {}
}
