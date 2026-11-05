<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Microphones and dictating quietly

Wispr Bro works with any microphone your Mac can use. This page explains
how the four common kinds compare, why some are better for whispering, and
how to get a quiet voice through. It is the page behind **More about
microphones** in the _Dictate quietly_ lesson (Help › Essentials › Learn to
dictate quietly, or **Show me how** on the home page).

Everything here runs on your Mac. Wispr Bro never sends your voice or your
answers to the quiz anywhere.

## The four kinds

| Microphone          | Mic quality | Whispering around others | Whispering at home | Price                   |
| ------------------- | ----------- | ------------------------ | ------------------ | ----------------------- |
| Wired headset       | Great       | Great                    | Great              | Low                     |
| Desk or USB mic     | Great       | Good                     | Great              | Medium                  |
| Wireless earbuds    | Good        | Good                     | Good               | Medium                  |
| Built-in microphone | Fair        | Fair                     | Good               | Comes with most laptops |

- **Wired headset.** The microphone sits on a short arm or on the cable,
  a few centimetres from your mouth. That distance is what matters most
  for a whisper: the quieter you speak, the closer the microphone has to
  be. A cable also means no radio link that can lower quality or drop out.
- **Desk or USB mic.** The clearest sound when you sit close to it at one
  desk. It stays on the desk, though, and in a busy room it also hears the
  people around you, so lean in rather than speak up.
- **Wireless earbuds.** Nothing to plug in, and they go everywhere. Their
  microphones sit near your ears rather than your mouth, and while the
  microphone is in use the wireless link often switches to a lower-quality
  mode. They work, but a whisper needs a little more breath.
- **Built-in microphone.** Always there on a laptop or an iMac, and fine at
  home. It is further from your mouth, so a whisper has to be a bit firmer
  and the room is picked up too. A Mac mini, Mac Studio or Mac Pro has no
  microphone of its own.

## How the quiz picks your best match

The lesson asks four questions: which microphone you would be comfortable
using, where you usually work, whether you move between places, and which
microphones you have. Each kind gets a score:

1. its mic quality;
2. how well it whispers around others, counted more the more you work
   around other people;
3. how well it whispers at home, counted more the more you work somewhere
   private;
4. if you move between places, a bonus for kinds that travel with you and a
   large penalty for a desk mic; if you stay at one desk, a small bonus for
   a desk mic;
5. a bonus for the kind you said you would be comfortable using;
6. a bonus for a microphone you already have; otherwise its price counts a
   little against it.

The highest score is the best match. A built-in microphone is never
recommended if you said your Mac has none. Questions you skip count for
nothing either way. The rules are in `src/fork/tutorials/micTable.ts` and
tested over every combination of answers.

## Whispering well

With any microphone, and especially the built-in one:

- **Sit close.** About 30 cm, roughly a forearm, from the microphone.
  Leaning in a little helps more than speaking louder.
- **Whisper with some breath.** Not the faintest whisper: a little firmer,
  as if talking to someone across a small table.
- **Keep a steady pace.** Say each word fully; don't let the ends of
  sentences fade away.
- **Let go last.** Release the dictation key only after your last word.

If words still go missing:

- raise the input volume in **System Settings › Sound › Input** (the
  lesson's **Open Sound settings** button goes there);
- check which microphone Wispr Bro uses in **Settings › Microphone**. A
  wired or built-in microphone is the most dependable; a wireless one can
  drop out.
