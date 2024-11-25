use std::collections::HashMap;

use lazy_static::lazy_static;
use teloxide::utils::markdown;

pub static DESC_COMMAND_SEARCH: &str = concat!(
    "Ricerca semplice: cerca all'interno di titoli e scontrini (descrizioni) degli episodi.\n",
    "Sintassi `/search {query}`.\n",
    "La query è case-insensitive.\n",
    "Es. \n",
    "- `/s pokemon` trova tutte le puntate con \"pokemon\" nel titolo o nella descrizione.\n",
    "- `/s green oaks` trova la puntata \"PPP Speciale: PGdR™ - Green Oaks\".",
);

pub static DESC_COMMAND_SEARCH_ADVANCED: &str = concat!(
    "Ricerca transcript: cerca all'interno della *trascrizione* della puntata, ovvero quello che viene pronunciato ",
    " dagli host in puntata. La ricerca viene effettuata su tutte le puntate. Una volta trovata la puntata utilizza ",
    "il comando /sae per cercare all'interno di una singola puntata.\n",
    "Sintassi `/sa {query}`.\n",
    "La query è case-insensitive. E supporta alcune keywords come google, ecco alcuni esempi: \n",
    "- `lorro -sio`: cerca tutte le puntate in cui viene detto \"lorro\" ed esclude quelle in cui viene detto \"sio\".\n",
    "- `nick sio`: cerca tutte le puntate in cui viene detto \"nick\" e quelle in cui viene detto \"sio\".\n",
    "- `\"nick lorro\"`: cerca tutte le puntate in cui viene detto \"nick\" e subito dopo \"lorro\".\n",
    "Es. se voglio cercare \"pokemon rosso\", devo scrivere `/sa \"pokemon rosso\"`, se scrivo `/sa pokemon rosso` la ",
    "ricerca sarà su tutte le puntate in cui viene detto \"pokemon\", ma anche **tutte** le puntate in cui viene detto \"rosso\"!.",
);

pub static DESC_COMMAND_SEARCH_ADVANCED_EPISODE: &str = concat!(
    "Ricerca testo del transcript di una puntata, fornisci il numero della puntata e il testo.\n",
    "Sintassi `/sae {episodio} {query}`.\n",
    "La query è case-insensitive. `{episodio}` può essere il numero dell'episodio, il titolo o il codice identificativo spreaker ",
    "(avanzato).\n",
    "La query non supporta le keywords di /sa. Ma supporta ricerca tramite regex (avanzato).\n",
    "Gli argomenti possono essere racchiusi tra virgolette `\"` per cercare frasi intere.\n",
    "Es.\n",
    "- `/sae 1 \"pokemon rosso\"`: cerca la frase \"pokemon rosso\" all'interno della puntata",
);

pub static WELCOME_STRING: &str = concat!(
    "Ciao! Sono il bot di PPP, posso aiutarti a trovare le puntate in cui si parla di un argomento specifico.",
);

/// Note: the footer string must be **markdown** formatted!
pub static FOOTER_STRING: &str = concat!(
    "Questo bot è sviluppato da @topongo ed è open\\-source\\! [topongo/ppp\\-bot](https://github.com/topongo/ppp\\-bot)",
);


lazy_static!{
        // (':', '.', '(', ')', '-', '!'].iter().cloned().collect();
    pub static ref ESCAPE_CHARS: HashMap<char, &'static str> = [
        (':', "\\:"),
        ('.', "\\."),
        ('(', "\\("),
        (')', "\\)"),
        ('-', "\\-"),
        ('!', "\\!"),
    ].iter().cloned().collect();
    pub static ref HELP_MESSAGE: String = format!(
        "{}\n\n{}\n\n{}",
        markdown::escape(WELCOME_STRING),
        [DESC_COMMAND_SEARCH, DESC_COMMAND_SEARCH_ADVANCED, DESC_COMMAND_SEARCH_ADVANCED_EPISODE]
            .iter()
            .map(|s| s
                .chars()
                .map(|c| ESCAPE_CHARS.get(&c).map(|c| c.to_string()).unwrap_or(c.to_string()))
                .collect::<String>()
            )
            .collect::<Vec<String>>()
            .join("\n\n"),
        FOOTER_STRING,
    );
}
