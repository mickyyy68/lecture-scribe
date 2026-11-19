//! macOS's spell checker, to tell ordinary words from misheard forms.
//!
//! AppKit's spell checker belongs to the main thread. In the app that thread runs the event
//! loop, so other threads hand it the work (`main_loop_started`); in the headless modes and the
//! tests nothing else uses AppKit, and it runs where it's called.
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

static MAIN_LOOP: AtomicBool = AtomicBool::new(false);

/// Called by the app once its main thread runs the event loop.
pub fn main_loop_started() {
    MAIN_LOOP.store(true, Ordering::SeqCst);
}

/// Whether `s` is ordinary text: every word of it spelled right in one of `langs` (ISO codes
/// such as "en", "it"; languages macOS has no dictionary for are skipped). "last name" and
/// "interact" are; "Cloudfail" and "HCB" aren't. The spell checker skips words in capitals and
/// words with digits or symbols, so those never count as ordinary here.
pub fn ordinary(s: &str, langs: &[&str]) -> bool {
    let plain = |w: &str| w.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '’') && !w.chars().skip(1).any(char::is_uppercase);
    if s.split_whitespace().next().is_none() || !s.split_whitespace().all(plain) {
        return false;
    }
    let (s, langs): (String, Vec<String>) = (s.to_string(), langs.iter().map(|l| l.to_string()).collect());
    on_main(move || check(&s, &langs))
}

fn check(s: &str, langs: &[String]) -> bool {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::{NSRange, NSString};
    if !s.chars().any(char::is_alphabetic) {
        return false;
    }
    objc2::rc::autoreleasepool(|_| unsafe {
        let Some(cls) = AnyClass::get(c"NSSpellChecker") else { return false };
        let sc: *mut AnyObject = msg_send![cls, sharedSpellChecker];
        if sc.is_null() {
            return false;
        }
        let avail: *mut AnyObject = msg_send![sc, availableLanguages];
        let text = NSString::from_str(s);
        langs.iter().any(|l| {
            let lang = NSString::from_str(l);
            let has: Bool = msg_send![avail, containsObject: &*lang];
            if !has.as_bool() {
                return false;
            }
            let r: NSRange = msg_send![sc, checkSpellingOfString: &*text, startingAt: 0isize, language: &*lang, wrap: Bool::NO, inSpellDocumentWithTag: 0isize, wordCount: std::ptr::null_mut::<isize>()];
            r.location == isize::MAX as usize // NSNotFound: nothing misspelled
        })
    })
}

/// Runs `f` on the main thread (see the module docs) and returns its result.
fn on_main<T: Send, F: FnOnce() -> T + Send>(f: F) -> T {
    unsafe extern "C" {
        fn pthread_main_np() -> i32;
        static _dispatch_main_q: c_void;
        fn dispatch_sync_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    }
    if !MAIN_LOOP.load(Ordering::SeqCst) || unsafe { pthread_main_np() } != 0 {
        return f();
    }
    struct Ctx<F, T> {
        f: Option<F>,
        out: Option<T>,
    }
    extern "C" fn work<T, F: FnOnce() -> T>(p: *mut c_void) {
        let c = unsafe { &mut *(p as *mut Ctx<F, T>) };
        c.out = Some((c.f.take().unwrap())());
    }
    let mut c = Ctx { f: Some(f), out: None };
    unsafe { dispatch_sync_f(&raw const _dispatch_main_q, &mut c as *mut Ctx<F, T> as *mut c_void, work::<T, F>) };
    c.out.unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordinary_words_are_told_from_misheard_forms() {
        for s in ["last name", "interact", "main server"] {
            assert!(super::ordinary(s, &["en", "it"]), "{s}");
        }
        assert!(super::ordinary("acqua", &["en", "it"]) && super::ordinary("Local Article", &["en"]));
        for s in ["Cloudfail", "BIRAM", "HCB", "AppStream", "takslet_lock", "dcs-resol.com", "Esen", "TSG"] {
            assert!(!super::ordinary(s, &["en", "it"]), "{s}");
        }
    }
}
