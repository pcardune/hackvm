declare class Output {
  static printString(s: string): void;
  /**
   * prints n starting at the cursor location
   * @param n
   */
  static printInt(n: number): void;

  /**
   * Advances the cursor to the beginning of the next line
   */
  static println(): void;
}

declare class Screen {
  static clearScreen(): void;
}
