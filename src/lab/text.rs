//! Lab lessons in every language. A missing field is a compile error, as in `i18n::Tr`.
//! These are `&'static str` in the binary: nothing here is allocated at runtime.

use crate::i18n::{Lang, lang};

pub struct StationText {
    pub name: &'static str,
    pub question: &'static str,
    /// Plain language, with an analogy.
    pub basic: &'static str,
    /// Protocol details.
    pub tech: &'static str,
    /// Hint about what to look at in the 3D scene.
    pub watch: &'static str,
    /// Labels of the live data rows, in the order `facts()` returns them.
    pub facts: &'static [&'static str],
}

pub struct LabText {
    pub title: &'static str,
    pub tour: &'static str,
    pub live: &'static str,
    pub basic: &'static str,
    pub technical: &'static str,
    pub play_tip: &'static str,
    pub watch: &'static str,
    pub real_data: &'static str,
    pub demo_banner: &'static str,
    pub sample_note: &'static str,
    pub hint_orbit: &'static str,
    pub look_light: &'static str,
    pub look_dark: &'static str,
    pub l_header: &'static str,
    pub l_rotors: &'static str,
    pub l_board: &'static str,
    pub nonce_plus: &'static str,
    pub avalanche_result: &'static str,
    pub live_title: &'static str,
    pub live_body: &'static str,
    pub ev_new_block: &'static str,
    pub ev_share: &'static str,
    pub ev_best: &'static str,
    pub ev_demo_block: &'static str,
    /// Header fields, in header order.
    pub fields: [&'static str; 6],
    pub s_tx: &'static str,
    pub s_coinbase: &'static str,
    pub s_merkle_root: &'static str,
    pub s_nonce_sweep: &'static str,
    pub s_hash_now: &'static str,
    pub s_best: &'static str,
    pub s_pool: &'static str,
    pub s_network: &'static str,
    pub s_you: &'static str,
    pub s_network_cloud: &'static str,
    pub s_one_in: &'static str,
    pub s_flipped: &'static str,
    pub s_next_block: &'static str,
    pub s_zero_bits: &'static str,
    pub stations: [StationText; 8],
}

pub fn text() -> &'static LabText {
    match lang() {
        Lang::Pt => &PT,
        Lang::En => &EN,
    }
}

static PT: LabText = LabText {
    title: "Como funciona a mineração",
    tour: "Tour guiado",
    live: "Ao vivo",
    basic: "Básico",
    technical: "Técnico",
    play_tip: "Avançar sozinho a cada 25 segundos",
    watch: "OBSERVE NA CENA",
    real_data: "AGORA, NO SEU MINERADOR",
    demo_banner: "DEMONSTRAÇÃO: sem mineração ativa, usando o bloco gênese de 2009",
    sample_note: "Amostra real: 1 hash a cada {}",
    hint_orbit: "Arraste para girar • Role para aproximar • ← → mudam a etapa",
    look_light: "Claro",
    look_dark: "Escuro",
    l_header: "CABEÇALHO · 80 BYTES",
    l_rotors: "SHA-256 × 2 · 64 RODADAS CADA",
    l_board: "HASH · 256 BITS",
    nonce_plus: "Testar o próximo nonce (+1)",
    avalanche_result: "Mudou 1 no nonce: {} de 256 bits viraram",
    live_title: "Tudo acontecendo ao mesmo tempo",
    live_body: "Esta é a linha completa, rodando com o trabalho que o seu minerador recebeu da pool.\n\nEventos reais aparecem no canto da cena: novo bloco na rede (a corrida recomeça), share aceita pela pool e novo recorde de dificuldade.\n\nClique em uma etapa acima para voltar a ela no tour.",
    ev_new_block: "Novo bloco na rede: a corrida recomeçou",
    ev_share: "Share aceita pela pool",
    ev_best: "Novo recorde de dificuldade: {}",
    ev_demo_block: "Bloco gênese encontrado! Nonce 2083236893, janeiro de 2009",
    fields: ["Versão", "Bloco anterior", "Raiz de Merkle", "Horário", "Alvo (bits)", "Nonce"],
    s_tx: "{} transações",
    s_coinbase: "coinbase: sua recompensa",
    s_merkle_root: "Raiz de Merkle",
    s_nonce_sweep: "Varredura dos 4,29 bilhões de nonces",
    s_hash_now: "hash agora",
    s_best: "seu recorde",
    s_pool: "alvo da pool",
    s_network: "alvo da rede",
    s_you: "VOCÊ",
    s_network_cloud: "A REDE",
    s_one_in: "1 em {}",
    s_flipped: "{} bits mudaram",
    s_next_block: "próximo bloco",
    s_zero_bits: "últimos hashes: bits zero de cada um",
    stations: [
        StationText {
            name: "Mempool",
            question: "De onde vêm as transações?",
            basic: "Toda transação de Bitcoin passa por uma sala de espera chamada mempool. Cada nó da rede mantém a sua.\n\nUm bloco tem espaço limitado, como um ônibus com poucos lugares. Por isso quem paga uma taxa maior por byte embarca primeiro. Quem monta o bloco escolhe as transações que rendem mais, e é daí que vem parte do lucro.",
            tech: "O bloco comporta 4 milhões de weight units (~1 MvB). A seleção ordena por feerate em sat/vB, considerando pacotes pai + filho (CPFP).\n\nNo solo via Stratum v1, quem monta o bloco é a pool: seu minerador recebe o resultado pronto, em `mining.notify`. Os números abaixo vêm da API do mempool.space.",
            watch: "Cada cubo é uma transação. A cor indica a taxa: de cinza (barata) a laranja (cara). As mais caras saem da fila e sobem para o bloco. Os cubos são ilustrativos; os números abaixo são reais.",
            facts: &["Transações na fila", "Tamanho da fila", "Taxa do próximo bloco", "Taxa para ~30 min"],
        },
        StationText {
            name: "Bloco candidato",
            question: "Como milhares de transações viram um resumo só?",
            basic: "Prepara-se um bloco candidato. A primeira transação é especial: a coinbase cria bitcoins novos e paga a recompensa. Aqui, para o seu endereço.\n\nDepois, as transações são resumidas duas a duas, como um torneio, até sobrar um único código: a raiz de Merkle. Mudou uma vírgula em qualquer transação, a raiz muda inteira.",
            tech: "Cada nó da árvore é SHA-256d(esquerda || direita). Um nível com número ímpar duplica o último item.\n\nA pool envia só o caminho da coinbase até a raiz (o `merkle_branch`). Com N ramos, o bloco tem até 2^N transações. O minerador monta coinb1 + extranonce1 + extranonce2 + coinb2, faz o hash e sobe a árvore: é o que `make_work` faz neste app.",
            watch: "As transações sobem nível por nível até a raiz. O cubo laranja na base é a coinbase.",
            facts: &["Ramos de Merkle", "Transações no bloco (até)", "Extranonce2", "Raiz de Merkle"],
        },
        StationText {
            name: "Cabeçalho",
            question: "O que exatamente é minerado?",
            basic: "O minerador não faz hash do bloco inteiro, só de um cabeçalho de 80 bytes.\n\nEle contém o hash do bloco anterior (que forma a corrente), a raiz de Merkle (que representa todas as transações), o horário, o alvo de dificuldade e um campo livre: o nonce.",
            tech: "Layout: version (4) | prev_hash (32) | merkle_root (32) | time (4) | bits (4) | nonce (4), com inteiros em little-endian.\n\nOs hashes são exibidos invertidos em relação à ordem interna. Como os primeiros 64 bytes não mudam entre nonces, o minerador guarda o estado do SHA-256 após eles (midstate) e economiza metade do primeiro hash.",
            watch: "São 80 colunas, uma por byte: a altura é o valor real do byte. O campo em laranja é o que está na lista abaixo. Só as 4 últimas colunas, as do nonce, mudam o tempo todo.",
            facts: &["Versão", "Bloco anterior", "Raiz de Merkle", "Horário", "Alvo (bits)", "Nonce"],
        },
        StationText {
            name: "Nonce",
            question: "O que muda a cada tentativa?",
            basic: "O nonce é um bilhete de loteria numerado. Mudou o número, é um bilhete novo.\n\nExistem 4,29 bilhões de nonces. Parece muito, mas a rede inteira esgota todos em uma fração de milissegundo. Por isso, ao fim da varredura, o minerador muda um detalhe na coinbase, ganha uma raiz de Merkle nova e recomeça com outro cabeçalho.",
            tech: "São 32 bits (0 a 2³²−1). Esgotado o espaço, incrementa-se o `extranonce2`, o que muda a coinbase, a raiz de Merkle e os primeiros 64 bytes do cabeçalho (novo midstate).\n\nO campo `time` e alguns bits de `version` (version rolling, BIP 320) também podem variar. Cada thread de CPU e cada GPU deste app pega um extranonce2 próprio, por isso não repetem trabalho.",
            watch: "Cada roda é um dígito hexadecimal do nonce, repetido no painel. As da direita giram mais rápido. A barra na frente mostra até onde a varredura chegou.",
            facts: &["Nonce atual", "Seu hashrate", "Tempo para varrer tudo", "Hashes testados"],
        },
        StationText {
            name: "SHA-256 duplo",
            question: "Dá para prever o resultado?",
            basic: "O cabeçalho passa duas vezes pelo SHA-256, um liquidificador matemático. Sai um número de 256 bits que parece aleatório.\n\nTeste o botão abaixo: ele muda o nonce em apenas 1. Cerca de metade dos bits vira, sem nenhum padrão. Isso é o efeito avalanche, e por causa dele não existe atalho nem estratégia: só testando.",
            tech: "SHA-256 tem 64 rodadas de rotações, XOR e somas módulo 2³² sobre blocos de 64 bytes. O hash duplo evita ataques de extensão de comprimento.\n\nCada bit de saída muda com probabilidade ~1/2 quando a entrada muda, então o número de bits alterados segue aproximadamente uma Binomial(256, ½): média 128, desvio ~8. Este app calcula SHA-256d na CPU e num compute shader WGSL na GPU.",
            watch: "A grade mostra os 256 bits do hash: claro é 1, escuro é 0. Ao testar o nonce +1, os bits que mudaram ficam laranja.",
            facts: &["Hash desta amostra", "Bits que mudaram com +1", "Hashes entre duas amostras"],
        },
        StationText {
            name: "Alvo",
            question: "Quando um hash é vencedor?",
            basic: "O hash é um número. Para valer, ele precisa ficar abaixo de um alvo definido pela rede, o que na prática significa começar com muitos zeros. É como exigir uma sequência de caras em cara ou coroa: cada zero a mais dobra o trabalho.\n\nA régua compara o hash de agora, o seu melhor resultado e os alvos da pool e da rede. Repare na distância que falta.",
            tech: "`bits` codifica o alvo em forma compacta. Dificuldade = alvo_dif1 / alvo, e o número esperado de hashes por bloco é D × 2³².\n\nZeros à esquerda são uma aproximação visual: o critério real é hash < alvo, comparando o número inteiro. A pool aceita shares com alvo bem mais fácil, o que prova que você trabalha, mas só um hash abaixo do alvo da rede cria um bloco.",
            watch: "As colunas mostram os últimos hashes amostrados, quase sempre com poucos zeros. As placas marcam cada nível na régua de zeros.",
            facts: &["Zeros do hash atual", "Zeros do seu recorde", "Zeros exigidos pela pool", "Zeros exigidos pela rede", "Dificuldade da rede"],
        },
        StationText {
            name: "Corrida",
            question: "Contra quem você está competindo?",
            basic: "Milhões de máquinas tentam ao mesmo tempo, e cada hash é um sorteio independente. Não há fila nem progresso acumulado: quem tem mais máquinas compra mais bilhetes, mas qualquer um pode acertar no próximo hash.\n\nÉ por isso que se chama loteria. Vitórias solo são raras, mas acontecem: de vez em quando um pequeno minerador acerta um bloco inteiro.",
            tech: "O tempo até um bloco segue uma distribuição exponencial: sem memória. Sua chance por hash é 1/(D × 2³²), e a chance em T segundos é 1 − exp(−H·T / (D·2³²)).\n\nPools dividem a recompensa para reduzir a variância; o valor esperado continua o mesmo, menos a taxa. Em solo você fica com o prêmio, mas a variância é máxima.",
            watch: "A esfera é a rede, e cada cubo representa 1/200 dela. O ponto laranja é você, desenhado muito maior do que deveria: em escala real, ele seria invisível.",
            facts: &["Seu hashrate", "Hashrate da rede", "Sua fatia da rede", "Chance por dia", "Tempo médio esperado"],
        },
        StationText {
            name: "Corrente",
            question: "O que acontece quando alguém ganha?",
            basic: "Quem acha um hash válido anuncia o bloco. Os nós conferem em instantes: é difícil achar, mas fácil verificar. Se estiver certo, o bloco entra na corrente e o minerador leva a recompensa.\n\nO hash desse bloco vira o \"bloco anterior\" do próximo. Todos jogam fora o que estavam fazendo e a corrida recomeça. Alterar um bloco antigo quebraria todos os seguintes, e é isso que protege o histórico.",
            tech: "A coinbase só pode ser gasta após 100 confirmações. O subsídio cai pela metade a cada 210.000 blocos (halving).\n\nA cada 2.016 blocos a dificuldade é reajustada para manter a média de 10 minutos por bloco, com limite de 4 vezes para cima ou para baixo. Quando dois blocos surgem ao mesmo tempo, vence a corrente com mais trabalho acumulado. No Stratum, `clean_jobs` manda os mineradores abandonarem o trabalho anterior.",
            watch: "Cada bloco se liga ao anterior pelo hash. Quando a rede acha um bloco de verdade, o último bloco acende e aparece um aviso.",
            facts: &["Altura do último bloco", "Minerado por", "Há quanto tempo", "Subsídio do bloco", "Próximo ajuste de dificuldade"],
        },
    ],
};

static EN: LabText = LabText {
    title: "How mining works",
    tour: "Guided tour",
    live: "Live",
    basic: "Basic",
    technical: "Technical",
    play_tip: "Advance by itself every 25 seconds",
    watch: "WATCH IN THE SCENE",
    real_data: "RIGHT NOW, ON YOUR MINER",
    demo_banner: "DEMONSTRATION: no active mining, using the 2009 genesis block",
    sample_note: "Real sample: 1 hash out of every {}",
    hint_orbit: "Drag to rotate • Scroll to zoom • ← → change the step",
    look_light: "Light",
    look_dark: "Dark",
    l_header: "HEADER · 80 BYTES",
    l_rotors: "SHA-256 × 2 · 64 ROUNDS EACH",
    l_board: "HASH · 256 BITS",
    nonce_plus: "Test the next nonce (+1)",
    avalanche_result: "Nonce changed by 1: {} of 256 bits flipped",
    live_title: "Everything happening at once",
    live_body: "This is the whole line, running on the work your miner received from the pool.\n\nReal events appear in the corner of the scene: a new block on the network (the race restarts), a share accepted by the pool, and a new difficulty record.\n\nClick a step above to go back to it in the tour.",
    ev_new_block: "New block on the network: the race restarted",
    ev_share: "Share accepted by the pool",
    ev_best: "New difficulty record: {}",
    ev_demo_block: "Genesis block found! Nonce 2083236893, January 2009",
    fields: ["Version", "Previous block", "Merkle root", "Time", "Target (bits)", "Nonce"],
    s_tx: "{} transactions",
    s_coinbase: "coinbase: your reward",
    s_merkle_root: "Merkle root",
    s_nonce_sweep: "Sweep of the 4.29 billion nonces",
    s_hash_now: "hash now",
    s_best: "your record",
    s_pool: "pool target",
    s_network: "network target",
    s_you: "YOU",
    s_network_cloud: "THE NETWORK",
    s_one_in: "1 in {}",
    s_flipped: "{} bits changed",
    s_next_block: "next block",
    s_zero_bits: "latest hashes: zero bits of each",
    stations: [
        StationText {
            name: "Mempool",
            question: "Where do transactions come from?",
            basic: "Every Bitcoin transaction goes through a waiting room called the mempool. Each network node keeps its own.\n\nA block has limited space, like a bus with few seats, so whoever pays a higher fee per byte boards first. Whoever builds the block picks the transactions that pay the most, and that is part of the income.",
            tech: "A block holds 4 million weight units (~1 MvB). Selection sorts by feerate in sat/vB, taking parent + child packages into account (CPFP).\n\nIn solo mining over Stratum v1 the pool assembles the block: your miner receives the finished result in `mining.notify`. The numbers below come from the mempool.space API.",
            watch: "Each cube is a transaction. Color shows the fee, from gray (cheap) to orange (expensive). The priciest leave the queue and rise into the block. The cubes are illustrative; the numbers below are real.",
            facts: &["Transactions waiting", "Queue size", "Next block fee", "Fee for ~30 min"],
        },
        StationText {
            name: "Candidate block",
            question: "How do thousands of transactions become one summary?",
            basic: "A candidate block is prepared. The first transaction is special: the coinbase creates new bitcoin and pays the reward. Here, to your address.\n\nTransactions are then summarized two by two, like a tournament, until a single code remains: the Merkle root. Change a comma in any transaction and the whole root changes.",
            tech: "Each tree node is SHA-256d(left || right). A level with an odd count duplicates its last item.\n\nThe pool sends only the path from the coinbase to the root (the `merkle_branch`). With N branches the block has up to 2^N transactions. The miner builds coinb1 + extranonce1 + extranonce2 + coinb2, hashes it and climbs the tree: that is what `make_work` does in this app.",
            watch: "Transactions climb level by level up to the root. The orange cube at the base is the coinbase.",
            facts: &["Merkle branches", "Transactions in block (up to)", "Extranonce2", "Merkle root"],
        },
        StationText {
            name: "Header",
            question: "What exactly gets mined?",
            basic: "Miners do not hash the whole block, only an 80-byte header.\n\nIt holds the hash of the previous block (which forms the chain), the Merkle root (which stands for all transactions), the time, the difficulty target and one free field: the nonce.",
            tech: "Layout: version (4) | prev_hash (32) | merkle_root (32) | time (4) | bits (4) | nonce (4), with little-endian integers.\n\nHashes are displayed reversed from their internal order. Since the first 64 bytes do not change between nonces, the miner keeps the SHA-256 state after them (the midstate) and saves half of the first hash.",
            watch: "There are 80 columns, one per byte: height is the real byte value. The orange field is the one highlighted in the list below. Only the last 4 columns, the nonce, keep changing.",
            facts: &["Version", "Previous block", "Merkle root", "Time", "Target (bits)", "Nonce"],
        },
        StationText {
            name: "Nonce",
            question: "What changes on each attempt?",
            basic: "The nonce is a numbered lottery ticket. New number, new ticket.\n\nThere are 4.29 billion nonces. That sounds like a lot, but the whole network runs through them in a fraction of a millisecond. So when a sweep ends, the miner changes a detail in the coinbase, gets a new Merkle root and starts over with another header.",
            tech: "It has 32 bits (0 to 2³²−1). Once exhausted, `extranonce2` is incremented, which changes the coinbase, the Merkle root and the first 64 bytes of the header (a new midstate).\n\nThe `time` field and some `version` bits (version rolling, BIP 320) can also vary. Each CPU thread and GPU in this app takes its own extranonce2, so they do not repeat work.",
            watch: "Each wheel is one hex digit of the nonce, repeated on the panel. The ones on the right spin faster. The bar in front shows how far the sweep has gone.",
            facts: &["Current nonce", "Your hashrate", "Time to sweep them all", "Hashes tested"],
        },
        StationText {
            name: "Double SHA-256",
            question: "Can the result be predicted?",
            basic: "The header goes through SHA-256 twice, a mathematical blender. Out comes a 256-bit number that looks random.\n\nTry the button below: it changes the nonce by just 1. About half of the bits flip, with no pattern. This is the avalanche effect, and because of it there is no shortcut or strategy: only trying.",
            tech: "SHA-256 runs 64 rounds of rotations, XORs and additions modulo 2³² over 64-byte blocks. Hashing twice prevents length-extension attacks.\n\nEach output bit flips with probability ~1/2 when the input changes, so the number of changed bits roughly follows a Binomial(256, ½): mean 128, deviation ~8. This app computes SHA-256d on the CPU and in a WGSL compute shader on the GPU.",
            watch: "The grid shows the hash's 256 bits: bright is 1, dark is 0. When you test nonce +1, the changed bits turn orange.",
            facts: &["Hash of this sample", "Bits changed by +1", "Hashes between two samples"],
        },
        StationText {
            name: "Target",
            question: "When does a hash win?",
            basic: "A hash is a number. To count, it must fall below a target set by the network, which in practice means starting with many zeros. It is like demanding a streak of heads on a coin: each extra zero doubles the work.\n\nThe ruler compares the current hash, your best result and the pool and network targets. Notice how far there is to go.",
            tech: "`bits` encodes the target in compact form. Difficulty = diff1_target / target, and the expected hashes per block are D × 2³².\n\nLeading zeros are a visual approximation: the real test is hash < target, comparing the whole number. The pool accepts shares at a far easier target, which proves you are working, but only a hash below the network target creates a block.",
            watch: "The columns are the latest sampled hashes, almost always with few zeros. The plates mark each level on the zero ruler.",
            facts: &["Zeros in current hash", "Zeros in your record", "Zeros the pool requires", "Zeros the network requires", "Network difficulty"],
        },
        StationText {
            name: "The race",
            question: "Who are you competing against?",
            basic: "Millions of machines try at once, and each hash is an independent draw. There is no queue and no accumulated progress: more machines buy more tickets, but anyone can hit on the next hash.\n\nThat is why it is called a lottery. Solo wins are rare, but they happen: every now and then a small miner lands an entire block.",
            tech: "Time to a block follows an exponential distribution: it has no memory. Your chance per hash is 1/(D × 2³²), and the chance over T seconds is 1 − exp(−H·T / (D·2³²)).\n\nPools split rewards to reduce variance; the expected value stays the same, minus the fee. Solo you keep the prize, with maximum variance.",
            watch: "The sphere is the network, and each cube stands for 1/200 of it. The orange dot is you, drawn far bigger than it should be: at true scale it would be invisible.",
            facts: &["Your hashrate", "Network hashrate", "Your slice of the network", "Chance per day", "Average expected time"],
        },
        StationText {
            name: "The chain",
            question: "What happens when someone wins?",
            basic: "Whoever finds a valid hash announces the block. Nodes check it in moments: hard to find, easy to verify. If it is right, the block joins the chain and the miner takes the reward.\n\nThat block's hash becomes the \"previous block\" of the next one. Everyone drops what they were doing and the race restarts. Changing an old block would break all the following ones, which is what protects the history.",
            tech: "The coinbase can only be spent after 100 confirmations. The subsidy halves every 210,000 blocks (halving).\n\nEvery 2,016 blocks difficulty is retargeted to keep the 10-minute average, limited to a factor of 4 either way. When two blocks appear at once, the chain with the most accumulated work wins. In Stratum, `clean_jobs` tells miners to abandon previous work.",
            watch: "Each block links to the previous one through its hash. When the network finds a real block, the last block lights up and a notice appears.",
            facts: &["Latest block height", "Mined by", "How long ago", "Block subsidy", "Next difficulty adjustment"],
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_have_the_same_shape() {
        for (a, b) in PT.stations.iter().zip(&EN.stations) {
            assert_eq!(a.facts.len(), b.facts.len(), "{}", a.name);
        }
        let n = |s: &str| s.matches("{}").count();
        assert_eq!(n(PT.sample_note), n(EN.sample_note));
        assert_eq!(n(PT.avalanche_result), n(EN.avalanche_result));
        assert_eq!(n(PT.ev_best), n(EN.ev_best));
    }
}
