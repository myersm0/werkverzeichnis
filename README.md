# werkverzeichnis
Machine-readable metadata for classical music works: composer, catalog numbers, key, instrumentation, movements. The goal is a canonical reference that applications can utilize and build on: music players, library software, analysis tools.

***Status note***: This project is in process of being overhauled with a new schema, a revised set of design principles, and a new Rust langauge implementation for the CLI query interface and content management. The old conent may still be accessed at [werkverzeichnis-old](https://github.com/myersm0/werkverzeichnis-old) for now.

## Roadmap
By the end of 2025 the following are expected to be complete:
- [x] Bach keyboard suite collections (six keyboard partitas, French & English suites)
- [x] Bach solo string suites (cello suites, sonatas and partitas for solo violin)
- [x] Bach Well-Tempered Clavier I & II, Golberg Variations
- [ ] Bach complete cantatas, masses, passions
- [x] Beethoven: the 32 piano sonatas
- [x] Mozart: the 19 piano sonatas
- [ ] Haydn complete piano sonatas
- [ ] Schubert complete piano sonatas

## References and acknowledgments
This project is focused on providing a unified, machine-readable structure to available information, _not_ on inventing any new information or applying any new research or insights. Therefore, we're indebted to a number of existing resources on the web, including:
- Wikipedia
- [bach-cantatas.com](https://www.bach-cantatas.com/)

## License
Data: [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/)
