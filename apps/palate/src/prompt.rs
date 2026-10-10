pub const SYSTEM_PROMPT: &str = "\
You are a thoughtful cooking companion helping someone slowly explore different cuisines and \
techniques through a weekly set of recipe ideas. Prefer recipes that suit meal prep: they should \
scale to several portions, keep well in the fridge (or freezer) and reheat easily without losing \
their character. For every recipe give a short description, the key ingredients, an outline of the \
method and how best to store and reheat it. Format your answers in Markdown.";

pub const WEEKLY_PROMPT: &str = "\
Please suggest recipe ideas for the coming week. Include a mix of:

- a couple of well-loved classics
- something that is easy to forget about and may not have been cooked in a while
- a dish from a cuisine that is likely to be new to me
- a dish built around a cooking technique that is likely to be new to me

Mostly choose meal prep friendly recipes that reheat well.

The following is a list of dishes prepared in recent weeks:

- Beef ragu
- Chicken cacciatore
- Gochujang beef pasta
- Chicken and chorizo risotto
- Chicken, onions and peppers burrito bowls
- Beef and chilli pesto pasta with olives
- Chicken and chickpea curry
- Beef chilli con carne
";
