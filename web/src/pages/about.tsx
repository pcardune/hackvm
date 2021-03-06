import Container from "react-bootstrap/Container";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import { Link } from "react-router-dom";
import demos from "../demos";
function AboutPage() {
  return (
    <Container>
      <Row>
        <Col md={{ span: 8, offset: 2 }}>
          <h3>About</h3>
          <p>
            I recently finished reading{" "}
            <a
              href="https://www.nand2tetris.org/book"
              target="_blank"
              rel="noopener noreferrer"
            >
              The Elements of Computing Systems
            </a>
            , which walks through building a complete computing system from
            logic gates all the way through to the implementation of a high
            level programming language and basic operating system. The book is
            part of a course called{" "}
            <a
              href="https://www.nand2tetris.org/"
              target="_blank"
              rel="noopener noreferrer"
            >
              Nand to Tetris
            </a>
            , created by Noam Nisan and Shimon Schocken, which is taught online
            and in person.
          </p>
          <p>
            In an effort to continue the fun, and learn some more stuff, I
            decided to recreate the virtual machine emulator program that comes
            with the book in a way that will run inside a web browser, without
            having to download anything. The emulator is written in{" "}
            <a
              href="https://www.rust-lang.org/"
              target="_blank"
              rel="noopener noreferrer"
            >
              Rust
            </a>{" "}
            and compiled to{" "}
            <a
              href="https://webassembly.org/"
              target="_blank"
              rel="noopener noreferrer"
            >
              Web Assembly
            </a>
            , with some javascript glue code to handle canvas rendering and
            keyboard events.
          </p>
          <p>
            Checkout some of the demo programs (which other people wrote)
            running on the emulator:
          </p>
          <ul>
            {Object.keys(demos).map((demoId) => (
              <li key={demoId}>
                <Link to={`/emulator/${demoId}`}>{demos[demoId].title}</Link> -{" "}
                {demos[demoId].description} By {demos[demoId].author}.
              </li>
            ))}
          </ul>
          <p>
            All the source code for the virtual machine implementation and this
            website is available{" "}
            <a
              href="https://github.com/pcardune/hackvm"
              target="_blank"
              rel="noopener noreferrer"
            >
              on github
            </a>
            .
          </p>
        </Col>
      </Row>
    </Container>
  );
}

export default AboutPage;
