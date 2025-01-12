/* -*- c++ -*- */
/*
 * Copyright 2021 The Regents of the University of California.
 *
 * This is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3, or (at your option)
 * any later version.
 *
 * This software is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this software; see the file COPYING.  If not, write to
 * the Free Software Foundation, Inc., 51 Franklin Street,
 * Boston, MA 02110-1301, USA.
 */


#ifndef _QA_SWAP_16_H_
#define _QA_SWAP_16_H_

#include <cppunit/TestCase.h>
#include <cppunit/extensions/HelperMacros.h>

namespace gr {
namespace sparsdr {

class qa_swap_16 : public CppUnit::TestCase
{
public:
    CPPUNIT_TEST_SUITE(qa_swap_16);
    CPPUNIT_TEST(t1);
    CPPUNIT_TEST_SUITE_END();

private:
    void t1();
};

} /* namespace sparsdr */
} /* namespace gr */

#endif /* _QA_SWAP_16_H_ */
