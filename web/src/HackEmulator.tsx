import { useState, useCallback } from "react";
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
  children?: React.ReactNode;
};
const HackEmulator = ({ urls, children }: HackEmulatorProps) => {
  const [speed, setSpeed] = useState(20000);
  const [numInstructions, setNumInstructions] = useState(0);

  const { loading, canvasRef, paused, setPaused, reset } = useHackMachine(
    urls,
    {
      paused: false,
      speed,
      onTick: useCallback((machine: RustHackMachine, elapsedTimeMs: number) => {
        setNumInstructions(machine.numCycles / 1000);
      }, []),
    }
  );

  const togglePause = () => setPaused(!paused);

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
            {loading ? (
              <span>
                <Spinner animation="border" role="status"></Spinner> Loading...
              </span>
            ) : (
              <p className="mt-1">
                step:{" "}
                <span className="font-monospace">
                  {Math.floor(numInstructions)}
                </span>
                k
              </p>
            )}
          </Col>
        </Row>
      </Col>
      <Col md={4}>
        {children}
        <Accordion>
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
        </Accordion>
      </Col>
    </Row>
  );
};

export default HackEmulator;
