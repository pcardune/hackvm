import Container from "react-bootstrap/Container";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import HackEmulator from "../HackEmulator";

import demos, { OSFiles } from "../demos";
import { useParams } from "react-router";

function EmulatorPage() {
  const { demoId = "pong" } = useParams<{ demoId: string }>();

  const demo = demos[demoId];
  const urls = demo ? [...demo.files, ...OSFiles] : [];

  return (
    <Container>
      <Row>
        <Col md={12}>
          <strong className="fs-5">{demo.title}</strong>
          <p>by {demo.author}</p>
        </Col>
      </Row>
      <HackEmulator urls={urls}>
        {demo.instructions && <p>{demo.instructions}</p>}
        <p>
          This demo program for the hack platform was written by {demo.author}.
          You can find the code for it <a href={demo.projectUrl}>here</a>.
        </p>
      </HackEmulator>
    </Container>
  );
}

export default EmulatorPage;
