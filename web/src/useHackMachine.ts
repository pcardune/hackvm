import { useEffect, useState, useRef, useCallback } from "react";
import RustHackMachine from "./RustHackMachine";
import RemoteFS from "./RemoteFS";
import safeInterval from "./safeInterval";

export default function useHackMachine(
  url: string[],
  {
    speed,
    onTick,
    paused: initPaused = true,
  }: {
    speed: number;
    onTick?: (machine: RustHackMachine, elapsedTimeMs: number) => void;
    paused?: boolean;
  }
) {
  const [loading, setLoading] = useState(false);
  const [machine, setMachine] = useState<RustHackMachine>();
  const [paused, setPaused] = useState(initPaused);

  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    setPaused(initPaused);
    setMachine(undefined);
    setLoading(true);
    const context = canvasRef.current?.getContext("2d");
    context &&
      context.clearRect(0, 0, context.canvas.width, context.canvas.height);

    (async () => {
      const fetched = await RemoteFS.get().getFiles(url);
      const vmFiles = fetched.map((fetchState) => {
        const parts = fetchState.url.split("/");
        const filename = parts[parts.length - 1];
        return { filename, text: fetchState.data };
      });
      setMachine(await RustHackMachine.create({ vmFiles }));
      setLoading(false);
    })();
  }, [url]);

  useEffect(() => {
    if (!machine) return;
    if (paused) return;
    const startTime = new Date().getTime();
    const computeInterval = safeInterval(() => {
      machine?.tick(speed);
    }, 0);
    let onTickInterval: ReturnType<typeof safeInterval>;
    if (onTick) {
      onTickInterval = safeInterval(
        () => onTick(machine, new Date().getTime() - startTime),
        1000 / 30
      );
    }
    return () => {
      onTickInterval && clearInterval(onTickInterval);
      clearInterval(computeInterval);
    };
  }, [machine, paused, onTick, speed]);

  useEffect(() => {
    if (paused) return;
    if (!machine) return;
    const context = canvasRef.current?.getContext("2d");
    if (!context) return;
    const renderInterval = safeInterval(() => {
      machine.drawScreen(context);
    }, 1000 / 30);
    return () => {
      clearInterval(renderInterval);
    };
  }, [paused, machine]);

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      machine?.setKeyboard(event);
    },
    [machine]
  );
  const onKeyUp = useCallback(() => machine?.setKeyboard(null), [machine]);

  useEffect(() => {
    if (paused) return;
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [paused, onKeyDown, onKeyUp]);

  const reset = () => {
    if (!machine) return;
    machine.reset();
    const context = canvasRef.current?.getContext("2d");
    context && machine.drawScreen(context);
    onTick && onTick(machine, 0);
  };

  return {
    machine,
    loading,
    canvasRef,
    paused,
    setPaused,
    reset,
  };
}
