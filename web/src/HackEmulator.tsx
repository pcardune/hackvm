import { useState, useCallback, useEffect } from "react";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import Form from "react-bootstrap/Form";
import Card from "react-bootstrap/Card";
import Accordion from "react-bootstrap/Accordion";
import IconButton from "./IconButton";
import Spinner from "react-bootstrap/Spinner";
import useHackMachine from "./useHackMachine";
import type RustHackMachine from "./RustHackMachine";

type HackEmulatorProps = {
  urls: string[];
  config: { speed: number };
  children?: React.ReactNode;
};
const HackEmulator = ({ urls, config, children }: HackEmulatorProps) => {
  const [speed, setSpeed] = useState(config.speed);
  const [numInstructions, setNumInstructions] = useState(0);
  const [vmState, setVMState] = useState("");

  const {
    loading,
    canvasRef,
    paused,
    setPaused,
    reset,
    machine,
  } = useHackMachine(urls, {
    paused: false,
    speed,
    onTick: useCallback((machine: RustHackMachine) => {
      setNumInstructions(machine.numCycles / 1000);
    }, []),
  });

  useEffect(() => {
    setSpeed(config.speed);
    setNumInstructions(0);
  }, [urls, config]);

  const togglePause = () => setPaused(!paused);
  const onClickTick = () => {
    if (!paused) {
      setPaused(true);
    }
    if (machine) {
      machine.tick(1);
      setVMState(machine.getVM().get_debug());
    }
  };

  useEffect(() => {
    if (!machine) return;
    if (paused) return;
    const timeout = setInterval(() => {
      setVMState(machine?.getVM().get_debug());
    }, 500);
    return () => {
      clearInterval(timeout);
    };
  }, [machine, paused]);

  return (
    <Row>
      <Col md={8}>
        <Row>
          <Col>
            <canvas
              ref={canvasRef}
              id="myCanvas"
              width="512"
              height="256"
              style={{ border: "1px solid black", width: "100%" }}
            />
          </Col>
        </Row>
        <Row className="justify-content-between">
          <Col xs="auto">
            <IconButton
              onClick={onClickTick}
              icon="play-fill"
              label="Tick"
              className="mr-2"
            />
            <IconButton
              onClick={togglePause}
              label={paused ? "Play" : "Pause"}
              icon={paused ? "play-fill" : "pause-fill"}
              disabled={loading}
              className="mr-2"
            />
            <IconButton
              icon="arrow-counterclockwise"
              label="Reset"
              onClick={reset}
              disabled={numInstructions <= 0}
              className="mr-2"
            />
          </Col>
          <Col xs="5">
            {loading && (
              <span>
                <Spinner animation="border" role="status"></Spinner> Loading...
              </span>
            )}
          </Col>
        </Row>
      </Col>
      <Col md={4}>
        {children}
        <Accordion defaultActiveKey="1">
          <Card>
            <Accordion.Toggle as={Card.Header} eventKey="0">
              Configuration
            </Accordion.Toggle>
            <Accordion.Collapse eventKey="0">
              <Card.Body>
                <Form>
                  <Form.Group>
                    <Form.Label>Steps / Cycle: {speed}</Form.Label>
                    <Form.Control
                      type="range"
                      min={500}
                      max={100000}
                      value={speed}
                      className="form-range"
                      onChange={(e) => setSpeed(parseInt(e.target.value))}
                    />
                  </Form.Group>
                </Form>
              </Card.Body>
            </Accordion.Collapse>
          </Card>
          <Card>
            <Accordion.Toggle as={Card.Header} eventKey="1">
              Internal VM State
            </Accordion.Toggle>
            <Accordion.Collapse eventKey="1">
              <Card.Body>
                <pre>{vmState}</pre>
              </Card.Body>
            </Accordion.Collapse>
          </Card>
        </Accordion>
      </Col>
    </Row>
  );
};

export default HackEmulator;
