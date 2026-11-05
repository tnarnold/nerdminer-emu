# Roadmap

Ideias aprovadas ou propostas, sem data. Marque aqui o que entrar em uma versão.

## Próximas

- **Consumo de energia (estimativa em watts).** Mostrar no app quantos watts a mineração consome, estimados a partir do uso configurado de CPU/GPU e do modelo detectado. É uma estimativa: deixar isso claro na interface. No macOS não há leitura de potência sem privilégios; em Linux/Windows avaliar RAPL e contadores do fabricante antes de depender de tabela de TDP.
- **FAQ de consumo no site.** Explicar o que o app consome (CPU, GPU, memória, energia, custo em kWh) e como reduzir, usando os números reais da estimativa acima.
- **Hashrate cortado nas telas.** A partir de 100 KH/s o primeiro dígito é cortado na caixa amarela.

## Depois

- **Barra de menus, fase 2:** painel popover com gráficos.
- **Barra de menus, fase 3:** notificações e recordes históricos.
- **Mais idiomas:** por exemplo, espanhol, em `src/i18n.rs` e `src/lab/text.rs`.
- **Manual em inglês:** traduzir os exemplos que ainda estão em português.
