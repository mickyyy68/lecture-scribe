use transcribe_cpp::*;
fn main() {
    init_backends_default().unwrap();
    for p in std::env::args().skip(1) {
        let m = Model::load(&p).unwrap();
        let c = m.capabilities().unwrap();
        println!("{} arch={} {:#?}", p, m.arch(), c);
        for f in [Feature::Pnc, Feature::Itn, Feature::Vocabulary, Feature::LongForm, Feature::ContextPrompt] {
            println!("  {:?}: {:?}", f, m.supports(f));
        }
    }
}
