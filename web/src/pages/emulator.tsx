import Container from "react-bootstrap/Container";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import HackEmulator from "../HackEmulator";

import demos, { OSFiles } from "../demos";
import { useParams } from "react-router";

function EmulatorPage() {
  const { demoId = "pong" } = useParams<{ demoId: string }>();

  const demo = demos[demoId];
  if (!demo) {
    return <Container>No demo found at this url.</Container>;
  }
  const urls = demo ? [...demo.files, ...OSFiles] : [];
  const defaultConfig = { speed: 20000 };
  const config =
    demo && demo.config ? { ...defaultConfig, ...demo.config } : defaultConfig;

  return (
    <Container>
      <Row>
        <Col md={12}>
          <strong className="fs-5">{demo.title}</strong> - {demo.description}{" "}
          <a href={demo.projectUrl} target="_blank" rel="noopener noreferrer">
            [source code]
          </a>
          <p>by {demo.author} </p>
        </Col>
      </Row>
      <HackEmulator urls={urls} config={config}>
        {demo.instructions && <p>{demo.instructions}</p>}
      </HackEmulator>
    </Container>
  );
}

export default EmulatorPage;
