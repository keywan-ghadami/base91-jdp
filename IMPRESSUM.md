# Impressum

*Anbieterkennzeichnung nach deutschem Recht. Die Seite steht auf Deutsch, weil
das Gesetz, das sie verlangt, ein deutsches ist. English readers: this is the
legally required provider identification, and the contact details in it are the
ones to use for anything about this repository.*

Dieses Repository und die daraus erzeugte Website sind ein privates Angebot ohne
Gewinnerzielungsabsicht. Sie enthalten eine Formatspezifikation, eine
prototypische Implementierung und die Messungen dazu und stellen alles für
Bildung, Forschung und freie Nutzung bereit.

## Anbieter

<address>
Keywan Ghadami<br>
Hirschstr. 15<br>
79235 Vogtsburg<br>
Deutschland
</address>

Telefon: <a href="tel:+4917620785913">0176 20785913</a><br>
E-Mail: <a href="mailto:keywan.ghadami@gmail.com">keywan.ghadami@gmail.com</a>

Angaben nach § 18 Abs. 1 MStV und § 5 DDG. Verantwortlich für sämtliche Inhalte
ist dieselbe Person.

## Streitbeilegung

Über diese Seite kommen keine Verträge zustande; § 36 VSBG greift damit nicht.
An einem Verfahren vor einer Verbraucherschlichtungsstelle nehme ich nicht teil.

## Verlinkte Inhalte

Verlinkt sind eigene Projekte, die auf fremden Plattformen liegen — GitHub,
crates.io, eigene Unterdomains —, und fremde Quellen, auf die sich die
Spezifikation bezieht. Die eigenen Projekte verantworte ich wie die Inhalte
dieser Seite. Für die Plattformen selbst — Oberfläche, Werbung,
Nutzungsbedingungen, Datenverarbeitung — gelten die Angaben ihrer Betreiber, und
für fremde Quellen die ihrer Urheber.

## Richtigkeit der Inhalte

Die Inhalte entstehen mit Sorgfalt und sind Arbeitsstände: die Spezifikation ist
ein Entwurf, die Implementierung ein Prototyp, und Beschreibungen von Verfahren,
Messungen und Analysen können unvollständig, überholt oder falsch sein. Sie sind
Information, keine Beratung und keine Zusicherung von Eigenschaften. Für die
Programme in diesem Repository gilt der Gewährleistungsausschluss der
[Lizenz](LICENSE). Für alles, was hier steht, hafte ich nach den allgemeinen
Gesetzen.

Hinweise auf Fehler und auf Rechtsverletzungen genügen formlos per E-Mail; ich
prüfe sie und ändere oder entferne den betroffenen Inhalt zügig.

## Einsatz von KI

Texte, Spezifikation und ein großer Teil des Programmcodes sind unter Einsatz
generativer KI entstanden und anschließend gegen Messungen geprüft worden.
Ausgewählt, geprüft und entschieden habe ich; die redaktionelle Verantwortung im
Sinne des Art. 50 Abs. 4 der KI-Verordnung (Verordnung (EU) 2024/1689) liegt bei
mir. Maschinell erzeugter Text klingt sicherer, als er ist — siehe oben.

Urheberrechtlich geschützt ist, was auf menschlicher Gestaltung beruht: Auswahl,
Aufbau und Fassung dieser Dokumente.

## Nutzung der Inhalte

Für den Quellcode gilt die Mozilla Public License 2.0, siehe
[LICENSE](LICENSE). Die Dokumente — Spezifikation, Messberichte, diese Website —
dürfen darüber hinaus für gemeinnützige, wissenschaftliche und Bildungszwecke
sowie für jede andere nicht kommerzielle Nutzung frei verwendet, kopiert und
bearbeitet werden, solange die Quelle genannt wird; für weitergehende Nutzung
genügt eine Anfrage. Eine unabhängige Implementierung der Spezifikation ist
ausdrücklich erwünscht und an keine Bedingung geknüpft. Maschinelle Auswertung,
auch Text und Data Mining im Sinne des § 44b UrhG, ist ausdrücklich gestattet.

## Verwendete Software

Die Website wird von [`site/build.py`](site/README.md) aus den Markdown-Dateien
dieses Repositories erzeugt und nutzt dabei
[python-markdown](https://python-markdown.github.io/) (BSD-Lizenz). Die
ausgelieferten Seiten laden nichts nach: kein JavaScript, keine externen
Schriften, keine Einbindungen Dritter, keine Cookies.

Die Implementierung in [`rust/`](rust/README.md) bindet optional
[Zstandard](https://github.com/facebook/zstd) (Yann Collet, Meta; BSD-Lizenz)
für die komprimierte Klasse ein; ohne diese Klasse hat sie keine Abhängigkeit.
Das Format baut auf [basE91](http://base91.sourceforge.net/) von Joachim Henke
auf, dessen Alphabet und Paarkodierung nachgebaut, nicht übernommen sind.
