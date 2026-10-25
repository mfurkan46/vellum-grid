use super::lexer::{Token, tokenize};
use super::{BinOp, Expr, FormulaError, functions};
use crate::Locale;
use crate::locale::EN;

pub fn parse(src: &str, locale: &Locale) -> Result<Expr, FormulaError> {
    let mut tokens = tokenize(src, locale)?;
    let depth = tokens.iter().fold(0i64, |depth, token| match token {
        Token::Open => depth + 1,
        Token::Close => depth - 1,
        _ => depth,
    });
    tokens.extend((0..depth).map(|_| Token::Close));
    let mut parser = Parser {
        tokens,
        pos: 0,
        locale,
    };
    let expr = parser.comparison()?;
    if parser.pos == parser.tokens.len() {
        Ok(expr)
    } else {
        Err(FormulaError::Syntax)
    }
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    locale: &'a Locale,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    fn binary(
        &mut self,
        ops: &[(&str, BinOp)],
        operand: fn(&mut Self) -> Result<Expr, FormulaError>,
    ) -> Result<Expr, FormulaError> {
        let mut left = operand(self)?;
        while let Some(Token::Op(symbol)) = self.peek() {
            let Some((_, op)) = ops.iter().find(|(s, _)| s == symbol) else {
                break;
            };
            let op = *op;
            self.pos += 1;
            let right = operand(self)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expr, FormulaError> {
        self.binary(
            &[
                ("=", BinOp::Eq),
                ("<>", BinOp::Ne),
                ("<", BinOp::Lt),
                ("<=", BinOp::Le),
                (">", BinOp::Gt),
                (">=", BinOp::Ge),
            ],
            Self::concat,
        )
    }

    fn concat(&mut self) -> Result<Expr, FormulaError> {
        self.binary(&[("&", BinOp::Concat)], Self::additive)
    }

    fn additive(&mut self) -> Result<Expr, FormulaError> {
        self.binary(&[("+", BinOp::Add), ("-", BinOp::Sub)], Self::term)
    }

    fn term(&mut self) -> Result<Expr, FormulaError> {
        self.binary(&[("*", BinOp::Mul), ("/", BinOp::Div)], Self::power)
    }

    fn power(&mut self) -> Result<Expr, FormulaError> {
        self.binary(&[("^", BinOp::Pow)], Self::percent)
    }

    fn percent(&mut self) -> Result<Expr, FormulaError> {
        let mut expr = self.unary()?;
        while self.peek() == Some(&Token::Op("%")) {
            self.pos += 1;
            expr = Expr::Percent(Box::new(expr));
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, FormulaError> {
        match self.peek() {
            Some(Token::Op("-")) => {
                self.pos += 1;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some(Token::Op("+")) => {
                self.pos += 1;
                Ok(Expr::Plus(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, FormulaError> {
        match self.next().ok_or(FormulaError::Syntax)? {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::Text(s) => Ok(Expr::Text(s)),
            Token::Error(e) => Ok(Expr::Error(e)),
            Token::Reference(sheet, reference) => Ok(Expr::Reference { sheet, reference }),
            Token::Structured(reference) => Ok(Expr::Structured(reference)),
            Token::Open => {
                let inner = self.comparison()?;
                match self.next() {
                    Some(Token::Close) => Ok(Expr::Paren(Box::new(inner))),
                    _ => Err(FormulaError::Syntax),
                }
            }
            Token::Word(word) if self.peek() == Some(&Token::Open) => {
                self.pos += 1;
                self.call(&word)
            }
            Token::Word(word) => Ok(
                match self.locale.parse_bool(&word).or(EN.parse_bool(&word)) {
                    Some(b) => Expr::Bool(b),
                    None => Expr::Name(word),
                },
            ),
            _ => Err(FormulaError::Syntax),
        }
    }

    fn call(&mut self, word: &str) -> Result<Expr, FormulaError> {
        let name = self.locale.canonical_function(word);
        let mut args = Vec::new();
        if self.peek() == Some(&Token::Close) {
            self.pos += 1;
        } else {
            loop {
                let arg = match self.peek() {
                    Some(Token::Sep | Token::Close) => Expr::Missing,
                    _ => self.comparison()?,
                };
                args.push(arg);
                match self.next() {
                    Some(Token::Sep) => {}
                    Some(Token::Close) => break,
                    _ => return Err(FormulaError::Syntax),
                }
            }
        }
        if let Some((min, max)) = functions::arity(&name) {
            let local = self.locale.function_name(&name).to_owned();
            if args.len() < min {
                return Err(FormulaError::TooFewArguments(local));
            }
            if args.len() > max {
                return Err(FormulaError::TooManyArguments(local));
            }
        }
        Ok(Expr::Call(name, args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TR;

    fn canonical(src: &str, locale: &Locale) -> String {
        parse(src, locale).unwrap().write(&EN)
    }

    #[test]
    fn respects_operator_precedence() {
        let expr = parse("-2^2", &EN).unwrap();
        assert!(matches!(expr, Expr::Binary(BinOp::Pow, ref a, _) if matches!(**a, Expr::Neg(_))));
        let expr = parse("1+2*3^2&\"x\"=A1", &EN).unwrap();
        let Expr::Binary(BinOp::Eq, left, _) = expr else {
            panic!()
        };
        let Expr::Binary(BinOp::Concat, sum, _) = *left else {
            panic!()
        };
        assert!(matches!(*sum, Expr::Binary(BinOp::Add, _, _)));
    }

    #[test]
    fn power_is_left_associative() {
        let expr = parse("2^3^2", &EN).unwrap();
        let Expr::Binary(BinOp::Pow, left, _) = expr else {
            panic!()
        };
        assert!(matches!(*left, Expr::Binary(BinOp::Pow, _, _)));
    }

    #[test]
    fn normalizes_case_and_spaces() {
        assert_eq!(canonical(" topla( a1 ; b2 ) ", &TR), "SUM(A1,B2)");
        assert_eq!(canonical("eğer(doğru;1;2)", &TR), "IF(TRUE,1,2)");
        assert_eq!(canonical("sum(a1:b2)", &EN), "SUM(A1:B2)");
    }

    #[test]
    fn handles_missing_and_empty_arguments() {
        assert_eq!(canonical("TODAY()", &EN), "TODAY()");
        assert_eq!(canonical("IF(A1,,2)", &EN), "IF(A1,,2)");
        assert_eq!(canonical("unknown(1)", &EN), "UNKNOWN(1)");
        assert_eq!(canonical("myname+1", &EN), "myname+1");
        assert_eq!(canonical("Vergi_Oranı*2", &TR), "Vergi_Oranı*2");
    }

    #[test]
    fn checks_argument_counts() {
        assert_eq!(
            parse("EĞER(A1)", &TR),
            Err(FormulaError::TooFewArguments("EĞER".into()))
        );
        assert_eq!(
            parse("UZUNLUK(A1;A2)", &TR),
            Err(FormulaError::TooManyArguments("UZUNLUK".into()))
        );
        assert_eq!(
            parse("TODAY(1)", &EN),
            Err(FormulaError::TooManyArguments("TODAY".into()))
        );
    }

    #[test]
    fn closes_missing_parentheses() {
        assert_eq!(canonical("TOPLA(B2:B5", &TR), "SUM(B2:B5)");
        assert_eq!(canonical("EĞER(A1>1;(A1+1", &TR), "IF(A1>1,(A1+1))");
    }

    #[test]
    fn rejects_broken_formulas() {
        for src in ["", "1+", "1)", "SUM(1+", "A1 B1", "*2", "SUM(1;2)", "(1))"] {
            assert!(parse(src, &EN).is_err(), "{src}");
        }
    }
}
