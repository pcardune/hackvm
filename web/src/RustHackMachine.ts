import type { WebVM } from "hackvm";

function getKeyValue(key: string) {
  const keyMap: Record<string, number> = {
    ArrowLeft: 130,
    ArrowUp: 131,
    ArrowRight: 132,
    ArrowDown: 133,
  };
  let value = 0;
  value = keyMap[key];
  if (value === undefined) {
    value = key.charCodeAt(0);
  }
  return value;
}

export default class RustHackMachine {
  static async create(program: {
    vmFiles: { filename: string; text: string }[];
  }): Promise<RustHackMachine> {
    const hack = await import("hackvm");
    hack.init_panic_hook();
    let machine;

    machine = hack.WebVM.new();
    for (let file of program.vmFiles) {
      machine.load_file(file.filename, file.text);
    }
    machine.init();

    return new RustHackMachine(machine);
  }

  private m: WebVM;
  private profile: boolean;
  private constructor(m: WebVM) {
    this.m = m;
    this.profile = false;
  }

  numCycles: number = 0;
  tick(n: number): void {
    if (this.profile) {
      this.m.tick_profiled(n);
    } else {
      this.m.tick(n);
    }
    this.numCycles += n;
  }
  reset(): void {
    this.m.reset();
    this.numCycles = 0;
    if (this.profile) {
      console.log(this.m.get_stats());
    }
    console.log(this.m.get_debug());
  }
  setKeyboard(event: { key: string } | null): void {
    this.m.set_keyboard(event ? getKeyValue(event.key) : 0);
  }
  drawScreen(ctx: CanvasRenderingContext2D): void {
    this.m.draw_screen(ctx);
  }
  getVM(): WebVM {
    return this.m;
  }
}
