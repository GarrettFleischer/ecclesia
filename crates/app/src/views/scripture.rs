//! ESV verses and the Crossway notice for the public landing.

use maud::{Markup, html};

/// One passage and the reference printed under it.
pub struct Verse {
    pub text: &'static str,
    pub reference: &'static str,
}

/// Acts 2:44-45.
pub const ACTS_2_44: Verse = Verse {
    text: "And all who believed were together and had all things in common. And they were selling their possessions and belongings and distributing the proceeds to all, as any had need.",
    reference: "Acts 2:44-45",
};

/// Acts 4:34.
pub const ACTS_4_34: Verse = Verse {
    text: "There was not a needy person among them.",
    reference: "Acts 4:34",
};

/// Acts 6:1, the complaint clause used on the landing.
pub const ACTS_6_1_NEGLECT: Verse = Verse {
    text: "A complaint by the Hellenists arose against the Hebrews because their widows were being neglected in the daily distribution.",
    reference: "Acts 6:1",
};

/// 1 Corinthians 12:27, opening clause on the landing.
pub const FIRST_CORINTHIANS_12_27_YOU: Verse = Verse {
    text: "You are the body of Christ and individually members of it.",
    reference: "1 Corinthians 12:27",
};

/// Galatians 6:2.
pub const GALATIANS_6_2: Verse = Verse {
    text: "Bear one another’s burdens, and so fulfill the law of Christ.",
    reference: "Galatians 6:2",
};

/// Galatians 6:10.
pub const GALATIANS_6_10: Verse = Verse {
    text: "So then, as we have opportunity, let us do good to everyone, and especially to those who are of the household of faith.",
    reference: "Galatians 6:10",
};

/// 1 Corinthians 12:26.
pub const FIRST_CORINTHIANS_12_26: Verse = Verse {
    text: "If one member suffers, all suffer together; if one member is honored, all rejoice together.",
    reference: "1 Corinthians 12:26",
};

/// 1 Corinthians 12:27.
pub const FIRST_CORINTHIANS_12_27: Verse = Verse {
    text: "Now you are the body of Christ and individually members of it.",
    reference: "1 Corinthians 12:27",
};

/// Crossway permission line for the ESV text.
pub const ESV_NOTICE: &str = "Scripture quotations are from the ESV® Bible (The Holy Bible, English Standard Version®), © 2001 by Crossway, a publishing ministry of Good News Publishers. ESV Text Edition: 2025. The ESV text may not be quoted in any publication made available to the public by a Creative Commons license. The ESV may not be translated in whole or in part into any other language. Used by permission. All rights reserved.";

/// `blockquote.verse` with the passage and its reference.
pub fn verse(passage: &Verse) -> Markup {
    verse_blockquote(passage, "verse")
}

fn verse_blockquote(passage: &Verse, class: &str) -> Markup {
    html! {
        blockquote class=(class) {
            p { (passage.text) }
            cite { (passage.reference) }
        }
    }
}
