# Owners, sections and categories (0.7.0)

## The model

- **Entities** are who or what things belong to: people, companies, vehicles, pets, homes
  (`property`), digital assets (an account, a website or app you track as a thing) and other.
- **Items** are what expires or comes back (a SOAT, a domain, a passport, a birthday). An item
  has **one or more owners**: `items.entity_id` is the main one (shown first, used for the
  reminder title) and `item_owners (item, entity, pos)` holds all of them, the main one at
  `pos 0`. `Item::owner_ids()` lists them, `Item::owned_by(e)` asks.
- **Entities own entities** through `relations` with role `owner` (`from` owns `to`): two
  people own a company, the company owns a car. `entity::owned_through(e)` follows those links
  at any depth (cycles are cut); `entity::owners_of(e)` lists the direct owners.
- **Archived entities** (`entities.archived`) leave the sidebar, the section pages, the pickers
  and the Map; their items and links stay, and their page says *Archived*.

## What a person "has"

`view::Filter::for_entity(e, relations)` matches an item when `e` owns it, or when something `e`
owns (through `owner` links) owns it. The list, the sidebar counts and the entity page use it;
rows reached through a company carry a *via Otherwise SAS* chip (`view::via`), and the entity
page groups them under *Through Otherwise SAS*.

## Sections and categories

Each category may belong to a section (`Category::section`): Vehicle → Vehicles, Company →
Companies, Home → Home, Digital → Digital, Identity and Birthdays & anniversaries → People;
insurance, contracts, warranties, memberships, health and other have none and show under their
owners. A section page shows its entities as cards **and** the items of its categories that no
entity of that kind holds (`view::loose`), each with its owners; the sidebar shows them as one
row under the section (*Domains, certificates and subscriptions · 7*). So a domain always shows
in Digital, whether a company, a person or a digital entity holds it, and an item's category and
the section it shows in agree.

Domains, certificates and subscriptions are the digital thing themselves: their holder is a
person or a company. The new-item form and the templates no longer preset a *digital* owner for
them; the kind of a new owner is guessed from the name (a legal suffix is a company, a domain
name is a digital entity, anything else a person) and can be changed.

## Migration 6 (from 0.6.x)

Forward-only and in one transaction, tested by `db::tests::version_6_owners_and_digital_twins`:

1. `item_owners` is created and every item's existing owner becomes its main owner there.
2. `entities.archived` is added (false for all).
3. Digital entities the old *Belongs to … New, as: Digital* flow made, recognisable because
   nobody described them (no identifier, details, year, country or notes, not shared) and their
   name is not a domain:
   - merged into the person or company **of the same name** when there is one: items (main and
     co-owned), links (a duplicate link or one to itself is dropped), a live .ics file and
     identifier / notes move over; only the empty duplicate is deleted;
   - else, when the name has a legal suffix (`Otherwise SAS`, `… LLC`), it becomes a company;
   - else it is left as it is (it may be a real account; archive or merge it from its page or
     with the assistant).
4. A domain item without a domain name gets its own name when that is a domain (`vcity.io`).

Nothing else changes; no item is deleted. Older Vigía versions refuse the migrated file
(*made by a newer Vigía*) instead of misreading it.
