#[allow(unused_imports)]
use pest::Parser;

#[derive(Parser)]
#[grammar = "fun.pest"]
pub struct FUNParser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comments() {
        FUNParser::parse(Rule::COMMENT, "// this is a comment").unwrap();
    }

    fn assert_all_bad(rule: Rule, bad: &[&str]) {
        bad.iter().for_each(|t| {
            assert!(
                FUNParser::parse(rule, t).is_err(),
                "Expected \"{}\" not to parse, but it did.",
                t
            );
        });
    }

    fn assert_all_good(rule: Rule, good: &[&str]) {
        good.iter().for_each(|t| {
            let s = FUNParser::parse(rule, t)
                .expect(&format!("\"{}\" failed to parse", t))
                .as_str();
            assert_eq!(s, *t);
        });
    }

    #[test]
    fn test_identifier() {
        let good = vec![
            "foo",
            "foo1",
            "_foo",
            "foo_bar",
            "foo_bar_baz_0",
            "FooBar",
            "fda",
        ];
        assert_all_good(Rule::identifier, &good);
        assert!(FUNParser::parse(Rule::identifier, "1foo").is_err());
        assert_eq!(
            FUNParser::parse(Rule::identifier, "foo").unwrap().as_str(),
            "foo"
        );
        assert_eq!(
            FUNParser::parse(Rule::identifier, "f_bar_baz o o")
                .unwrap()
                .as_str(),
            "f_bar_baz"
        );
    }

    #[test]
    fn test_let_statement() {
        let good = vec!["let foo: number = 34;", "let   foo:number=34;"];
        let bad = vec!["letfoo:number=34;"];
        assert_all_good(Rule::let_statement, &good);
        assert_all_bad(Rule::let_statement, &bad);
    }

    #[test]
    fn test_return_statement() {
        let good = vec!["return 0;", "return 3 + 4;"];
        let bad = vec!["returnfoo;"];
        assert_all_good(Rule::return_statement, &good);
        assert_all_bad(Rule::return_statement, &bad);
    }

    #[test]
    fn test_parameter_decl() {
        let good = vec!["()", "(x: number, y: number)", "(x: number)"];
        let bad = vec!["(", "(x, y)", "(x: number y:number)", "(x:number,)"];
        assert_all_good(Rule::parameter_decl, &good);
        assert_all_bad(Rule::parameter_decl, &bad);
    }

    #[test]
    fn test_expr() {
        let good = vec!["a+b"];
        assert_all_good(Rule::expr, &good);
    }

    #[test]
    fn test_file() {
        let result = FUNParser::parse(
            Rule::file,
            "
class Vector {
    
    x: number;
    y: number;
    color: string;

    constructor(){
        let foo:number = 1;
        this.color = \"red\";
    }

    magnitude():number {
        let bar:number = 2;
        return Math.sqrt((this.x * this.x) + this.y * this.y);
    }

    add(other: Vector): Vector {
        let a: bool = true;
        return new Vector(
            this.x + other.x,
            this.y + other.y
        );
    }
}

class PolyLine {
    points: Vector[];

    push(point: Vector): void {
        this.points.push(point);
    }

    line_length(): number {
        let length: number = 0;
        let i: number = 0;
        while (i < this.points.length) {
            // let v1: Vector = this.points[i];
            return foo;
        }
    }
}
",
        )
        .unwrap();
        println!("Here is what I got: {:?}", result);
    }
}
