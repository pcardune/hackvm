/// <reference path="./types.ts" />
class Main {
  static main(): void {
    Output.printString('   Hello World!');
    let v1: Vector = new Vector(10, 10);
    Output.printString(' x: ');
    Output.printInt(v1.x);
    Output.printString(' y: ');
    Output.printInt(v1.y);
    Output.printString(' mag: ');
    Output.printInt(v1.magnitude());
    Output.println();
    let mag: number = v1.magnitude();
    let max: number = 1;
    if (mag < max) {
      Output.printString('Less than ');
      Output.printInt(max);
    } else {
      Output.printString('At least ');
      Output.printInt(max);
    }
    Output.println();
    Output.printString('How about that?');
  }
}

class Vector {
  x: number;
  y: number;
  constructor(x: number, y: number) {
    this.x = x;
    this.y = y;
  }
  magnitude(): number {
    return Math.sqrt(this.x * this.x + this.y * this.y);
  }
}
