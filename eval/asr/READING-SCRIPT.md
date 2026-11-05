<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RS-09 reading script

Reference texts for the speech-model benchmark (RS-09) and the long-take
chunking test (DEC-51). The texts were written for this project. The
recordings contain the reader's voice, so they are personal data: they stay
in `~/wb-bench/audio/` on the reader's Mac and are never committed.

## How to record

1. Open **QuickTime Player** → File → **New Audio Recording**. Click the small
   arrow next to the record button and pick your usual microphone (the
   MacBook's built-in mic is fine).
2. Make **one recording per section** below (6 in total). Press record, read
   the whole section, press stop, then File → Save into the folder
   `wb-bench/audio` in your home folder, with the name in the section title
   (for example `en-sentences.m4a`).
3. Read the way you would dictate a message: natural speed, normal voice.
   Pause for about a second between sentences. If you stumble, just repeat the
   sentence and keep going. Don't restart the recording.
4. Read numbers, dates and email addresses the way you'd naturally say them.

Six recordings, about 15 minutes in total.

---

## 1. `en-sentences.m4a` (about 2 minutes)

1. Hey Sarah, can we move tomorrow's meeting to three thirty?
2. I just sent the invoice to accounting, it should arrive before Friday.
3. Please order twelve boxes of paper and two black ink cartridges.
4. The flight leaves on the fourteenth of March at seven forty five in the morning.
5. My email address is anna dot devries at example dot com.
6. Let's ship the update on Tuesday, no wait, on Wednesday.
7. The new laptop costs about eighteen hundred euros including tax.
8. Could you review the pull request and leave a comment on the README?
9. I'm running about ten minutes late, sorry, the train got stuck outside Utrecht.
10. Remind me to call the dentist next week and book a cleaning.
11. The quarterly revenue grew by roughly seven percent compared to last year.
12. Thanks for your help today, I really appreciate it.

## 2. `nl-sentences.m4a` (about 2 minutes)

1. Hoi Thomas, kunnen we de afspraak van morgen verzetten naar half vier?
2. Ik heb de factuur net naar de boekhouding gestuurd, die komt voor vrijdag aan.
3. Wil je twaalf dozen papier en twee zwarte inktpatronen bestellen?
4. De vlucht vertrekt op veertien maart om kwart voor acht 's ochtends.
5. We zetten de update dinsdag live, nee wacht, woensdag.
6. De nieuwe laptop kost ongeveer achttienhonderd euro inclusief btw.
7. Kun je de pull request bekijken en een opmerking bij de README zetten?
8. Ik ben een minuut of tien later, sorry, de trein stond stil bij Utrecht.
9. Herinner me eraan om volgende week de tandarts te bellen.
10. De omzet is dit kwartaal met ongeveer zeven procent gegroeid.
11. Het weer is vandaag eindelijk lekker, zullen we buiten lunchen?
12. Bedankt voor je hulp vandaag, ik waardeer het echt.

## 3. `fr-sentences.m4a` (about 2 minutes)

1. Salut Claire, est-ce qu'on peut déplacer la réunion de demain à quinze heures trente ?
2. Je viens d'envoyer la facture à la comptabilité, elle devrait arriver avant vendredi.
3. Peux-tu commander douze boîtes de papier et deux cartouches d'encre noire ?
4. Le vol part le quatorze mars à sept heures quarante-cinq du matin.
5. On publie la mise à jour mardi, non attends, mercredi.
6. Le nouvel ordinateur coûte environ mille huit cents euros TTC.
7. Tu peux relire la pull request et laisser un commentaire sur le README ?
8. J'ai à peu près dix minutes de retard, désolé, le train est bloqué avant Bruxelles.
9. Rappelle-moi d'appeler le dentiste la semaine prochaine.
10. Le chiffre d'affaires a augmenté d'environ sept pour cent ce trimestre.
11. Il fait enfin beau aujourd'hui, on déjeune dehors ?
12. Merci pour ton aide aujourd'hui, j'apprécie vraiment.

## 4. `en-long.m4a` (about 5 minutes, read straight through)

This tests long dictations. Read the whole text in one go, the way you would
dictate a long note. Short pauses between paragraphs are fine.

> Here are my notes from this morning's planning session, so I don't forget
> anything before the weekend.
>
> First, the launch date. We agreed to aim for the second week of December,
> but only if the beta testers stop finding serious bugs by the end of
> November. If they don't, we push it to January and say so openly. Nobody
> wants a rushed release right before the holidays, least of all the support
> team.
>
> Second, onboarding. The current flow asks people too many questions. The
> goal is that someone downloads the app, opens it, grants two permissions,
> and is dictating within two minutes. Every extra screen costs us users, so
> each one has to earn its place. Maria will draft a shorter version and we
> will test it with five people who have never seen the app before.
>
> Third, performance. On older laptops the first dictation after a restart
> still takes too long. Part of that is loading the speech model, and part of
> it is the cleanup step. We discussed loading things in the background while
> the person is still talking, which should hide most of the delay. Daniel
> thinks we can get the first result under one second on most machines, and
> under two seconds on the slowest ones we support.
>
> Fourth, languages. We have good results in English, Dutch and French, but
> German and Spanish need more testing. We also want to handle people who
> switch languages in the middle of a sentence, which happens more often than
> you'd think, especially in Belgium and Switzerland. For now the plan is to
> detect the language per dictation and not per word.
>
> Fifth, privacy. Everything stays on the computer. No recordings, no text
> and no usage numbers leave the machine, and we want to be able to prove
> that, not just promise it. We will publish a simple page that explains
> exactly which two websites the app ever talks to and why: one for updates
> and one for downloading the models.
>
> Sixth, the budget. We are slightly over on design and slightly under on
> infrastructure, so overall we're fine. I still want a short summary every
> month, nothing fancy, just the numbers and one paragraph of explanation.
>
> Finally, a few small things. The meeting room on the third floor is booked
> every Thursday from now on. The coffee machine is broken again, and someone
> promised to call the repair company, but nobody remembers who. And please,
> everyone, update your calendars, because half of the team still has the old
> schedule from September.
>
> Next steps: Maria sends the onboarding draft by Wednesday, Daniel shares the
> performance numbers on Friday, and I will write up the privacy page over the
> weekend. We meet again on Monday at ten.

## 5. `nl-long.m4a` (about 1 minute)

> Even een korte samenvatting van het gesprek van vanmiddag. We hebben
> besloten om de nieuwe versie pas in januari uit te brengen, omdat de
> testers nog te veel fouten vinden. Sanne gaat de introductie van de app
> korter maken, want mensen haken nu af voordat ze ook maar één keer hebben
> gedicteerd. Verder willen we beter testen op oudere laptops, omdat de eerste
> opname daar soms meer dan drie seconden duurt. Over de privacy waren we het
> snel eens: alles blijft op de computer en we leggen precies uit met welke
> twee websites de app contact maakt. Volgende week dinsdag om tien uur
> spreken we weer af, en dan wil ik graag de eerste cijfers zien.

## 6. `fr-long.m4a` (about 1 minute)

> Petit résumé de la réunion de cet après-midi. Nous avons décidé de sortir
> la nouvelle version seulement en janvier, parce que les testeurs trouvent
> encore trop d'erreurs. Julien va raccourcir l'introduction de l'application,
> car les gens abandonnent avant même d'avoir dicté une seule fois. Nous
> voulons aussi mieux tester sur les ordinateurs plus anciens, où le premier
> enregistrement prend parfois plus de trois secondes. Pour la
> confidentialité, tout le monde était d'accord : tout reste sur l'ordinateur
> et nous expliquons exactement avec quels deux sites l'application
> communique. On se revoit mardi prochain à dix heures, et j'aimerais voir
> les premiers chiffres à ce moment-là.
